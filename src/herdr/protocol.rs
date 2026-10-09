use crate::model::Agent;
use serde::Deserialize;
#[derive(Deserialize)]
struct Envelope<T> {
    id: String,
    result: T,
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum AgentsResult {
    #[serde(rename = "agent_list")]
    List { agents: Vec<Agent> },
}
pub fn parse_agents(bytes: &[u8], id: &str) -> anyhow::Result<Vec<Agent>> {
    let e: Envelope<AgentsResult> = serde_json::from_slice(bytes)?;
    anyhow::ensure!(e.id == id, "mismatched request identity");
    let AgentsResult::List { agents } = e.result;
    Ok(agents)
}
#[derive(Debug, Clone, Deserialize)]
pub struct Plugin {
    pub plugin_id: String,
    pub enabled: bool,
    pub plugin_root: String,
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum PluginsResult {
    #[serde(rename = "plugin_list")]
    List { plugins: Vec<Plugin> },
}
pub fn parse_plugins(bytes: &[u8], id: &str) -> anyhow::Result<Vec<Plugin>> {
    let e: Envelope<PluginsResult> = serde_json::from_slice(bytes)?;
    anyhow::ensure!(e.id == id, "mismatched request identity");
    let PluginsResult::List { plugins } = e.result;
    Ok(plugins)
}
#[derive(Deserialize)]
#[serde(tag = "type")]
enum Ping {
    #[serde(rename = "pong")]
    Pong { version: String },
}
pub fn parse_ping(bytes: &[u8], id: &str) -> anyhow::Result<String> {
    let e: Envelope<Ping> = serde_json::from_slice(bytes)?;
    anyhow::ensure!(e.id == id, "mismatched ping");
    let Ping::Pong { version } = e.result;
    anyhow::ensure!(version == "0.9.3", "unqualified Herdr version");
    Ok(version)
}
