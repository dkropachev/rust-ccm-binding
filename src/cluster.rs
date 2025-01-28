use std::collections::HashSet;
use std::{fs};
use std::fs::File;
use futures::future::join_all;
use std::io::{BufRead, BufReader, Error};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Command};
use crate::cluster_config::ScyllaConfig;

pub enum NodeStatus {
    ACTIVE,
    DELETED,
}

pub enum NodeStartOption {
    NOWAIT,
    WaitOtherNotice,
    WaitForBinaryProto,
}

pub(crate) struct Node {
    pub name: String,
    pub datacenter_id: i32,
    pub node_id: i32,
    pub status: NodeStatus,
    pub scylla: bool,
    pub smp: i32,
    pub memory: i32,
    // pub config: *const ScyllaConfig,
}


impl Node {
    fn log_command(command: &str, args: &[&str]) {
        println!("Executing command: {} {}", command, args.join(" "));
    }

    pub fn new(datacenter_id: i32, node_id: i32, scylla: bool, smp: i32, memory: i32) -> Self {
        Node {
            name: format!("node_{}_{}", datacenter_id, node_id),
            datacenter_id,
            node_id,
            status: NodeStatus::ACTIVE,
            scylla,
            smp,
            memory: {
                if memory != 0 {
                    memory
                } else {
                    512 * smp
                }
            },
        }
    }

    fn jmx_port(&self) -> i32 {
        7000 + self.datacenter_id * 100 + self.node_id
    }

    fn debug_port(&self) -> i32 {
        2000 + self.datacenter_id * 100 + self.node_id
    }

    fn scylla_extra_opts(&self) -> String {
        format!("--smp={} --memory={}M", self.smp, self.memory)
    }

    pub async fn init(&self) -> Result<(), Error> {
        let datacenter = self.datacenter_id.to_string();
        let jmx_port = self.jmx_port().to_string();
        let debug_port = self.debug_port().to_string();
        let mut args: Vec<&str> = vec!["add", &self.name, "--data-center", &datacenter, "--jmx-port", &jmx_port , "--remote-debug-port", &debug_port];
        if self.scylla {
            args.push("--scylla");
        }
        Self::log_command("ccm", &args);
        Command::new("ccm")
            .envs(vec![
                ("SCYLLA_EXTRA_OPTS", self.scylla_extra_opts()),
            ])
            .args(args)
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            })
    }

    pub async fn start(&self, opts: Option<&[NodeStartOption]>) -> Result<(), Error> {
        let mut args = vec!["start", &self.name];
        for opt in opts.unwrap_or(&[]) {
            match opt {
                NodeStartOption::NOWAIT => args.push("--no-wait"),
                NodeStartOption::WaitOtherNotice => args.push("--wait-other-notice"),
                NodeStartOption::WaitForBinaryProto => args.push("--wait-for-binary-proto"),
            }
        }
        Self::log_command("ccm", &args);
        Command::new("ccm")
            .envs(vec![
                ("SCYLLA_EXTRA_OPTS", self.scylla_extra_opts()),
            ])
            .args(args)
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            })
    }

    pub async fn delete(&mut self) -> Result<(), Error> {
        let args = ["remove", &self.name];
        Self::log_command("ccm", &args);
        Command::new("ccm")
            .args(args)
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    self.status = NodeStatus::DELETED;
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            })
    }
}

/// Represents a cluster instance managed by CCM.
pub(crate) struct Cluster {
    pub name: String,
    pub scylla: bool,
    pub version: String,
    pub ip_prefix: String,
    pub install_directory: String,
    nodes: Vec<Node>,
    destroyed: bool,
    pub default_node_smp: i32,
    pub default_node_memory: i32,
    pub default_node_config: Option<ScyllaConfig>,
}

#[cfg(test)]
impl Drop for Cluster {
    fn drop(&mut self) {
        if !self.destroyed {
            self.destroyed = true;
            Self::log_command("ccm", &["remove", &self.name]);
            let _ = Command::new("ccm")
                .args(["remove", &self.name])
                .stdout(Stdio::null()) // Redirect stdout to /dev/null
                .stderr(Stdio::null()) // Redirect stderr to /dev/null
                .spawn();
        }
    }
}

impl Cluster {
    fn log_command(command: &str, args: &[&str]) {
        println!("Executing command: {} {}", command, args.join(" "));
    }

    pub (crate) fn set_default_node_memory(&mut self, memory: i32) {
        self.default_node_memory = memory;
    }

    pub (crate) fn set_default_node_smp(&mut self, smp: i32) {
        self.default_node_smp = smp;
    }

    pub (crate) fn set_default_node_config(&mut self, config: ScyllaConfig) {
        self.default_node_config = config.into();
    }

    fn sniff_ip_prefix() -> Result<String, Error> {
        let mut used_ips = HashSet::new();

        if let Ok(file) = File::open("/proc/net/tcp") {
            let reader = BufReader::new(file);
            for line in reader.lines().skip(1) {
                if let Ok(line) = line {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(ip_hex) = parts.get(1) {
                        let ip_port: Vec<&str> = ip_hex.split(':').collect();
                        if let Some(ip_hex) = ip_port.get(0) {
                            let ip = u32::from_str_radix(ip_hex, 16).unwrap();
                            let ip = format!(
                                "{}.{}.{}.{}",
                                ip & 0xFF,
                                (ip >> 8) & 0xFF,
                                (ip >> 16) & 0xFF,
                                (ip >> 24) & 0xFF
                            );
                            used_ips.insert(ip);
                        }
                    }
                }
            }
        }

        for a in 1..=255 {
            let ip_prefix = format!("127.0.{}", a);
            let mut available = true;
            for b in 1..=255 {
                let ip = format!("{}.{}", ip_prefix, b);
                if used_ips.contains(&ip) {
                    available = false;
                    break;
                }
            }
            if available {
                return Ok(ip_prefix);
            }
        }
        Err(Error::from_raw_os_error(1))
    }

    pub fn get_free_node_id(&self, datacenter_id: i32) -> i32 {
        'outer: for node_id in 1..=255 {
            for node in self.nodes.iter() {
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

    pub(crate) fn add_node(
        & mut self,
        datacenter_id: Option<i32>,
    ) -> &Node {
        let node = Node::new(
            datacenter_id.unwrap_or(1),
            self.get_free_node_id(datacenter_id.unwrap_or(1)),
            self.scylla,
            self.default_node_memory,
            self.default_node_smp,
            // self.default_node_config.or(),
        );
        self.nodes.push(node);
        self.nodes.last().expect("Failed to add node")
    }

    const DEFAULT_MEMORY: i32 = 0;
    const DEFAULT_SMP: i32 = 1;

    pub(crate) fn create(
        name: String,
        version: String,
        ip_prefix: Option<&str>,
        number_of_nodes: Vec<i32>,
        install_directory: String,
        scylla: bool,
    ) -> Result<Self, Error> {
        let mut ip_prefix =
            ip_prefix.map_or_else(|| Self::sniff_ip_prefix(), |v| Ok(v.to_string()))?;
        if !ip_prefix.ends_with(".") {
            ip_prefix = format!("{}.", ip_prefix);
        }

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
        };

        for datacenter_id in 0..number_of_nodes.len() {
            for _ in 0..number_of_nodes[datacenter_id] {
                _ = cluster.add_node(Some(datacenter_id as i32))
            }
        }
        Ok(cluster)
    }

    pub(crate) async fn init(&self) -> Result<(), Error> {
        let ccm_path = PathBuf::from(format!(
            "{}/.ccm/{}",
            std::env::var("HOME").unwrap(),
            self.name
        ));

        if ccm_path.exists() {
            fs::remove_dir_all(&ccm_path)?;
        }
        let mut args: Vec<&str> = vec![
            "create",
            &self.name,
            "-v",
            &self.version,
            "-i",
            &self.ip_prefix,
        ];
        if self.scylla {
            args.push("--scylla");
        }
        Self::log_command("ccm", &args);
        let res = Command::new("ccm")
            .args(args)
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            });

        if res.is_err() {
            return res;
        }

        join_all(self.nodes.iter().map(|obj| obj.init())).await.into_iter().collect()
    }

    pub(crate) async fn start(&self, opts: Option<&[NodeStartOption]>) -> Result<(), Error> {
        join_all(self.nodes.iter().map(|obj| obj.start(opts))).await.into_iter().collect()
    }

    pub(crate) async fn stop(&self) -> Result<(), Error> {
        Self::log_command("ccm", &["stop", &self.name]);
        Command::new("ccm")
            .args(["stop", &self.name])
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            })
    }

    pub(crate) async fn destroy(mut self) -> Result<(), Error> {
        self.destroyed = true;
        Self::log_command("ccm", &["remove", &self.name]);
        Command::new("ccm")
            .args(["remove", &self.name])
            .status()
            .await
            .map_err(|e| e.into())
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(Error::from_raw_os_error(1))
                }
            })
    }
}

#[tokio::test]
async fn test_cluster_lifecycle() {
    let mut cluster = Cluster::create(
        "test_cluster".to_string(),
        "release:6.2".to_string(),
        None,
        vec![3],
        "/tmp/ccm".to_string(),
        true,
    )
        .expect("Failed to create cluster");
    cluster.init().await.expect("Failed to initialize cluster");
    cluster.start(None).await.expect("Failed to start cluster");
    let node = cluster.add_node(Some(1));
    node.init().await.expect("Failed to initialize node");
    node.start(None).await.expect("Failed to start node");
    cluster.stop().await.expect("Failed to stop cluster");
    cluster.destroy().await.expect("Failed to destroy cluster");
}
