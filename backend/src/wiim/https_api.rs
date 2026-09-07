use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct HttpsApiClient {
    http: reqwest::Client,
    base_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpsApiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("collector URL error: {0}")]
    Url(#[from] url::ParseError),
    #[error("API returned failure: {0}")]
    ApiFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub index: u32,
    pub param_name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EqState {
    pub enabled: bool,
    pub preset_name: String,
    pub bands: Vec<EqBand>,
    pub channel_mode: Option<String>,
    pub source_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusEx {
    pub source: Option<String>,
    pub rssi: Option<i32>,
    pub ssid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusExRaw {
    #[serde(rename = "mode", default)]
    mode: Option<String>,
    #[serde(rename = "source", default)]
    source: Option<String>,
    #[serde(rename = "RSSI", default)]
    rssi: Option<i32>,
    #[serde(rename = "ssid", default)]
    ssid: Option<String>,
}

/// Raw response from EQGetBand / EQLoad
#[derive(Debug, Deserialize)]
struct EqBandResponse {
    #[serde(rename = "EQStat", default)]
    eq_stat: serde_json::Value,
    #[serde(rename = "Name", default)]
    name: Option<String>,
    #[serde(rename = "EQBand", default)]
    eq_band: Vec<EqBand>,
    #[serde(rename = "channelMode", default)]
    channel_mode: Option<String>,
    #[serde(rename = "source_name", default)]
    source_name: Option<String>,
}

impl EqBandResponse {
    fn into_state(self) -> EqState {
        let enabled = match &self.eq_stat {
            serde_json::Value::String(s) => s == "On",
            serde_json::Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
            _ => false,
        };
        EqState {
            enabled,
            preset_name: self.name.unwrap_or_default(),
            bands: self.eq_band,
            channel_mode: self.channel_mode,
            source_name: self.source_name,
        }
    }
}

impl HttpsApiClient {
    pub fn new(base_url: String, bearer_token: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {bearer_token}"))
                .expect("invalid collector bearer token"),
        );
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .default_headers(headers)
            .build()
            .expect("failed to build collector HTTPS client");
        Self { http, base_url }
    }

    /// Check if the HTTPS API is reachable.
    pub async fn probe(&self) -> bool {
        match self.command("getStatusEx").await {
            Ok(body) => body.contains("uuid"),
            Err(_) => false,
        }
    }

    async fn command(&self, cmd: &str) -> Result<String, HttpsApiError> {
        let mut url = reqwest::Url::parse(&self.base_url)?;
        url.query_pairs_mut().append_pair("command", cmd);
        debug!("HTTPS API: {}", url);
        let resp = self.http.get(url).send().await?;
        let text = resp.text().await?;
        if text == "unknown command" {
            return Err(HttpsApiError::ApiFailed(text));
        }
        Ok(text)
    }

    pub async fn eq_get_list(&self) -> Result<Vec<String>, HttpsApiError> {
        let text = self.command("EQGetList").await?;
        let presets: Vec<String> = serde_json::from_str(&text)?;
        Ok(presets)
    }

    pub async fn eq_get_band(&self) -> Result<EqState, HttpsApiError> {
        let text = self.command("EQGetBand").await?;
        let raw: EqBandResponse = serde_json::from_str(&text)?;
        Ok(raw.into_state())
    }

    pub async fn eq_load(&self, name: &str) -> Result<EqState, HttpsApiError> {
        let text = self.command(&format!("EQLoad:{name}")).await?;
        let raw: EqBandResponse = serde_json::from_str(&text)?;
        Ok(raw.into_state())
    }

    pub async fn eq_on(&self) -> Result<(), HttpsApiError> {
        let text = self.command("EQOn").await?;
        self.check_status(&text)
    }

    pub async fn eq_off(&self) -> Result<(), HttpsApiError> {
        let text = self.command("EQOff").await?;
        self.check_status(&text)
    }

    pub async fn eq_set_band(&self, index: u32, value: f64) -> Result<(), HttpsApiError> {
        let json = serde_json::json!({"index": index, "value": value});
        let text = self.command(&format!("EQSetBand:{json}")).await?;
        self.check_status(&text)
    }

    pub async fn eq_save(&self, name: &str) -> Result<(), HttpsApiError> {
        let text = self.command(&format!("EQSave:{name}")).await?;
        self.check_status(&text)
    }

    pub async fn eq_del(&self, name: &str) -> Result<(), HttpsApiError> {
        let text = self.command(&format!("EQDel:{name}")).await?;
        self.check_status(&text)
    }

    pub async fn get_channel_balance(&self) -> Result<f64, HttpsApiError> {
        let text = self.command("getChannelBalance").await?;
        text.trim()
            .parse::<f64>()
            .map_err(|_| HttpsApiError::ApiFailed(format!("bad balance value: {text}")))
    }

    pub async fn set_channel_balance(&self, balance: f64) -> Result<(), HttpsApiError> {
        let text = self
            .command(&format!("setChannelBalance:{balance}"))
            .await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    pub async fn get_crossfade(&self) -> Result<bool, HttpsApiError> {
        let text = self.command("GetFadeFeature").await?;
        #[derive(Deserialize)]
        struct Fade {
            #[serde(rename = "FadeFeature")]
            fade_feature: u8,
        }
        let parsed: Fade = serde_json::from_str(&text)?;
        Ok(parsed.fade_feature != 0)
    }

    pub async fn set_crossfade(&self, enabled: bool) -> Result<(), HttpsApiError> {
        let val = if enabled { "1" } else { "0" };
        let text = self.command(&format!("SetFadeFeature:{val}")).await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    pub async fn switch_source(&self, source: &str) -> Result<(), HttpsApiError> {
        let text = self
            .command(&format!("setPlayerCmd:switchmode:{source}"))
            .await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    pub async fn get_status_ex(&self) -> Result<StatusEx, HttpsApiError> {
        let text = self.command("getStatusEx").await?;
        let raw: StatusExRaw = serde_json::from_str(&text)?;
        Ok(StatusEx {
            source: raw.mode.or(raw.source),
            rssi: raw.rssi,
            ssid: raw.ssid,
        })
    }

    // ── Volume (avoids SOAP group-sync crosstalk) ──────────────

    pub async fn set_volume(&self, volume: u32) -> Result<(), HttpsApiError> {
        let text = self.command(&format!("setPlayerCmd:vol:{volume}")).await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    pub async fn set_mute(&self, mute: bool) -> Result<(), HttpsApiError> {
        let val = if mute { "1" } else { "0" };
        let text = self.command(&format!("setPlayerCmd:mute:{val}")).await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    // ── Multiroom ──────────────────────────────────────────────

    /// Tell this device to join a group as a slave of the given master IP.
    pub async fn join_group_master(&self, master_ip: &str) -> Result<(), HttpsApiError> {
        let text = self
            .command(&format!(
                "ConnectMasterAp:JoinGroupMaster:eth{master_ip}:wifi0.0.0.0"
            ))
            .await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    /// Return this device to standalone mode when it is stuck as a follower.
    pub async fn leave_group(&self) -> Result<(), HttpsApiError> {
        let text = self.command("ConnectMasterAp:JoinGroupMaster:eth0").await?;
        self.check_status(&text)
    }

    /// Dissolve the complete group led by this device.
    pub async fn ungroup(&self) -> Result<(), HttpsApiError> {
        let text = self.command("multiroom:Ungroup").await?;
        self.check_status(&text)
    }

    /// Kick a slave (by IP) from this device's group. Must be called on the master.
    pub async fn slave_kickout(&self, slave_ip: &str) -> Result<(), HttpsApiError> {
        let text = self
            .command(&format!("multiroom:SlaveKickout:{slave_ip}"))
            .await?;
        if text.trim() == "OK" {
            Ok(())
        } else {
            Err(HttpsApiError::ApiFailed(text))
        }
    }

    fn check_status(&self, text: &str) -> Result<(), HttpsApiError> {
        #[derive(Deserialize)]
        struct Status {
            status: String,
        }
        if let Ok(s) = serde_json::from_str::<Status>(text) {
            if s.status == "OK" {
                return Ok(());
            }
            return Err(HttpsApiError::ApiFailed(s.status));
        }
        // Some commands return just "OK"
        if text.trim() == "OK" {
            return Ok(());
        }
        Err(HttpsApiError::ApiFailed(text.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn commands_use_the_fixed_collector_route_and_bearer() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0_u8; 4096];
            let length = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..length]);
            assert!(request
                .starts_with("GET /wiim/wiim-1/linkplay?command=EQLoad%3AOffice+EQ HTTP/1.1"));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer airwave-test-token"));
            stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nOK")
                .await
                .unwrap();
        });
        let client = HttpsApiClient::new(
            format!("http://{address}/wiim/wiim-1/linkplay"),
            "airwave-test-token",
        );
        assert_eq!(client.command("EQLoad:Office EQ").await.unwrap(), "OK");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn multiroom_commands_use_bounded_recovery_payloads() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let expected = [
                "command=ConnectMasterAp%3AJoinGroupMaster%3Aeth192.0.2.10%3Awifi0.0.0.0",
                "command=ConnectMasterAp%3AJoinGroupMaster%3Aeth0",
                "command=multiroom%3AUngroup",
            ];
            for expected_query in expected {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = vec![0_u8; 4096];
                let length = stream.read(&mut request).await.unwrap();
                let request = String::from_utf8_lossy(&request[..length]);
                assert!(request.lines().next().unwrap().contains(expected_query));
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nOK",
                    )
                    .await
                    .unwrap();
            }
        });
        let client = HttpsApiClient::new(
            format!("http://{address}/wiim/wiim-1/linkplay"),
            "airwave-test-token",
        );

        client.join_group_master("192.0.2.10").await.unwrap();
        client.leave_group().await.unwrap();
        client.ungroup().await.unwrap();
        server.await.unwrap();
    }
}
