use crate::ccm_cli::{LoggedCmd, RunOptions};
use crate::cluster_config::ScyllaConfig;
use crate::run_options;
use std::collections::{HashMap, HashSet};
use std::io::Error as IoError;
use std::io::ErrorKind::DirectoryNotEmpty;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::{File, metadata};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;

pub enum NodeStatus {
    ACTIVE,
    DELETED,
}

pub enum NodeStartOption {
    NOWAIT,
    WaitOtherNotice,
    WaitForBinaryProto,
}

#[derive(Debug, Error)]
#[error("Multiple errors occurred: {0:?}")]
struct AggregatedError(Vec<String>);

pub(crate) struct Node {
    pub name: String,
    pub datacenter_id: i32,
    pub node_id: i32,
    pub status: NodeStatus,
    pub scylla: bool,
    pub smp: i32,
    pub memory: i32,
    pub config: ScyllaConfig,
    logged_cmd: Arc<LoggedCmd>,
    install_directory: String,
}

impl Node {
    pub fn new(
        datacenter_id: i32,
        node_id: i32,
        scylla: bool,
        smp: i32,
        memory: i32,
        config: ScyllaConfig,
        logged_cmd: Arc<LoggedCmd>,
        install_directory: String,
    ) -> Self {
        Node {
            name: format!("node_{}_{}", datacenter_id, node_id),
            datacenter_id,
            node_id,
            status: NodeStatus::ACTIVE,
            scylla,
            smp,
            memory: { if memory != 0 { memory } else { 512 * smp } },
            config,
            logged_cmd,
            install_directory,
        }
    }

    fn jmx_port(&self) -> i32 {
        7000 + self.datacenter_id * 100 + self.node_id
    }

    fn debug_port(&self) -> i32 {
        2000 + self.datacenter_id * 100 + self.node_id
    }

    fn get_ccm_env(&self) -> HashMap<String, String> {
        let mut env: HashMap<String, String> = HashMap::new();
        env.insert(
            "SCYLLA_EXT_OPTS".to_string(),
            format!("--smp={} --memory={}M", self.smp, self.memory),
        );
        env
    }

    pub async fn init(&self) -> Result<(), IoError> {
        let datacenter = format!("dc{}", self.datacenter_id);
        let jmx_port = self.jmx_port().to_string();
        let debug_port = self.debug_port().to_string();
        let mut args: Vec<&str> = vec![
            "add",
            &self.name,
            "--data-center",
            &datacenter,
            "--jmx-port",
            &jmx_port,
            "--remote-debug-port",
            &debug_port,
            "--config-dir",
            &self.install_directory,
        ];
        if self.scylla {
            args.push("--scylla");
        }

        self.logged_cmd
            .run_command("ccm", &args, run_options!(env = self.get_ccm_env()))
            .await?;
        Ok(())
    }

    pub async fn start(&self, opts: Option<&[NodeStartOption]>) -> Result<(), IoError> {
        let mut args = vec!["start", &self.name, "--config-dir", &self.install_directory];
        for opt in opts.unwrap_or(&[]) {
            match opt {
                NodeStartOption::NOWAIT => args.push("--no-wait"),
                NodeStartOption::WaitOtherNotice => args.push("--wait-other-notice"),
                NodeStartOption::WaitForBinaryProto => args.push("--wait-for-binary-proto"),
            }
        }

        self.logged_cmd
            .run_command("ccm", &args, run_options!(env = self.get_ccm_env()))
            .await?;
        Ok(())
    }

    pub async fn delete(&mut self) -> Result<(), IoError> {
        let args = ["remove", &self.name];
        self.logged_cmd.run_command("ccm", &args, None).await?;
        self.status = NodeStatus::DELETED;
        Ok(())
    }

    fn mark_deleted(&mut self) {
        self.status = NodeStatus::DELETED;
    }
}

/// Represents a cluster instance managed by CCM.
pub(crate) struct Cluster {
    pub name: String,
    pub scylla: bool,
    pub version: String,
    pub ip_prefix: String,
    pub install_directory: String,
    nodes: Vec<Arc<RwLock<Node>>>,
    destroyed: bool,
    pub default_node_smp: i32,
    pub default_node_memory: i32,
    pub default_node_config: Option<ScyllaConfig>,
    logged_cmd: Arc<LoggedCmd>,
}

#[cfg(test)]
impl Drop for Cluster {
    fn drop(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async { self.destroy().await.ok() });
        }
    }
}

impl Cluster {
    pub(crate) fn set_default_node_memory(&mut self, memory: i32) {
        self.default_node_memory = memory;
    }

    pub(crate) fn set_default_node_smp(&mut self, smp: i32) {
        self.default_node_smp = smp;
    }

    pub(crate) fn set_default_node_config(&mut self, config: ScyllaConfig) {
        self.default_node_config = config.into();
    }

    async fn sniff_ip_prefix() -> Result<String, IoError> {
        let mut used_ips = HashSet::new();
        let file = File::open("/proc/net/tcp").await?;
        let mut lines = BufReader::new(file).lines();
        while let Some(line) = lines.next_line().await? {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(ip_hex) = parts.get(1) {
                let ip_port: Vec<&str> = ip_hex.split(':').collect();
                if let Some(ip_hex) = ip_port.get(0) {
                    if let Some(ip) = u32::from_str_radix(ip_hex, 16).ok() {
                        used_ips.insert(format!(
                            "{}.{}.{}.",
                            ip & 0xFF,
                            (ip >> 8) & 0xFF,
                            (ip >> 16) & 0xFF,
                        ));
                    }
                }
            }
        }

        for a in 1..=255 {
            for b in 1..=255 {
                let ip_prefix = format!("127.{}.{}.", a, b);
                if !used_ips.contains(&ip_prefix) {
                    return Ok(ip_prefix);
                }
            }
        }
        Err(IoError::from_raw_os_error(1))
    }

    pub async fn get_free_node_id(&self, datacenter_id: i32) -> i32 {
        'outer: for node_id in 1..=255 {
            for node in self.nodes.iter() {
                let node = node.read().await;
                if node.datacenter_id == datacenter_id {
                    if node.node_id == node_id {
                        continue 'outer;
                    }
                }
            }
            return node_id;
        }
        256
    }

    pub(crate) async fn add_node(&mut self, datacenter_id: Option<i32>) -> &Arc<RwLock<Node>> {
        let dc = datacenter_id.unwrap_or(1);
        let node = Node::new(
            dc,
            self.get_free_node_id(dc).await,
            self.scylla,
            self.default_node_smp,
            self.default_node_memory,
            self.default_node_config.clone().unwrap_or_default(),
            self.logged_cmd.clone(),
            self.install_directory.clone(),
        );
        self.nodes.push(Arc::new(RwLock::new(node)));
        self.nodes.last().clone().unwrap()
    }

    const DEFAULT_MEMORY: i32 = 512;
    const DEFAULT_SMP: i32 = 1;

    pub(crate) async fn new(
        name: String,
        version: String,
        ip_prefix: Option<&str>,
        number_of_nodes: Vec<i32>,
        install_directory: String,
        scylla: bool,
    ) -> Result<Self, IoError> {
        let mut ip_prefix = match ip_prefix {
            Some(v) => v.to_string(),
            None => Self::sniff_ip_prefix().await?,
        };
        if !ip_prefix.ends_with(".") {
            ip_prefix = format!("{}.", ip_prefix);
        }

        match metadata(install_directory.as_str()).await {
            Ok(mt) => {
                if !mt.is_dir() {
                    return Err(IoError::new(
                        DirectoryNotEmpty,
                        format!("{install_directory} already exists and it is not a dictionary"),
                    ));
                }
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    tokio::fs::create_dir_all(install_directory.as_str()).await?;
                }
                _ => {
                    return Err(e.into());
                }
            },
        }

        let mut lcmd = LoggedCmd::new();
        lcmd.set_log_file(format!("{install_directory}/{name}.ccm.log"))
            .await?;

        let mut cluster = Cluster {
            name,
            scylla,
            version,
            ip_prefix,
            install_directory,
            destroyed: false,
            nodes: vec![],
            default_node_memory: Self::DEFAULT_MEMORY,
            default_node_smp: Self::DEFAULT_SMP,
            default_node_config: None,
            logged_cmd: Arc::new(lcmd),
        };

        for datacenter_id in 0..number_of_nodes.len() {
            for _ in 0..number_of_nodes[datacenter_id] {
                cluster.add_node(Some((datacenter_id + 1) as i32)).await;
            }
        }
        Ok(cluster)
    }

    pub(crate) async fn init(&self) -> Result<(), IoError> {
        let ccm_path = PathBuf::from(format!("{}/{}", self.install_directory, self.name));

        if ccm_path.exists() {
            tokio::fs::remove_dir_all(&ccm_path).await?;
        }
        let mut args: Vec<&str> = vec![
            "create",
            &self.name,
            "-v",
            &self.version,
            "-i",
            &self.ip_prefix,
            "--config-dir",
            &self.install_directory,
        ];
        if self.scylla {
            args.push("--scylla");
        }
        self.logged_cmd.run_command("ccm", &args, None).await?;

        for node in self.nodes.iter() {
            let node = Arc::clone(node);
            let node = node.read().await;
            node.init().await?;
        }

        Ok(())
    }

    pub(crate) async fn start(&self, opts: Option<&[NodeStartOption]>) -> Result<(), IoError> {
        for node in self.nodes.iter() {
            let node = node.read().await;
            node.start(opts).await?;
        }
        Ok(())
    }

    pub(crate) async fn stop(&mut self) -> Result<(), IoError> {
        if self.destroyed {
            return Ok(());
        }
        match self
            .logged_cmd
            .run_command(
                "ccm",
                &["stop", &self.name, "--config-dir", &self.install_directory],
                None,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub(crate) async fn destroy(&mut self) -> Result<(), IoError> {
        if self.destroyed {
            return Ok(());
        }
        self.stop().await.ok();
        match self
            .logged_cmd
            .run_command(
                "ccm",
                &[
                    "remove",
                    &self.name,
                    "--config-dir",
                    &self.install_directory,
                ],
                None,
            )
            .await
        {
            Ok(_) => {
                self.destroyed = true;
                // for mut node in self.nodes {
                //     node.mark_deleted();
                // }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[tokio::test]
async fn test_cluster_lifecycle() {
    let mut cluster = Cluster::new(
        "test_cluster".to_string(),
        "release:6.2".to_string(),
        None,
        vec![3],
        "/tmp/ccm".to_string(),
        true,
    )
    .await
    .expect("Failed to create cluster");

    cluster.init().await.expect("Failed to initialize cluster");
    cluster.start(None).await.expect("Failed to start cluster");
    {
        let node = cluster.add_node(Some(2)).await.write().await;
        node.init().await.expect("Failed to initialize node");
        node.start(None).await.expect("Failed to start node");
    }
    cluster.stop().await.expect("Failed to stop cluster");
    cluster.destroy().await.expect("Failed to destroy cluster");
}
