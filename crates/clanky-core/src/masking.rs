//! One policy captured by the server; every output stream has its own matcher.
use super::*;
use clanky_mask::Masker;

pub(super) struct Policy {
    environment: Vec<(String, String)>,
    invalid_environment: bool,
    extra: Mutex<Vec<(String, String)>>,
}
impl Policy {
    pub fn capture() -> Self {
        let mut environment = Vec::new();
        let mut invalid_environment = false;
        for (name, value) in std::env::vars_os() {
            let upper = name.to_string_lossy().to_ascii_uppercase();
            if !["KEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD"]
                .iter()
                .any(|word| upper.contains(word))
            {
                continue;
            }
            match (name.into_string(), value.into_string()) {
                (Ok(name), Ok(value)) => environment.push((name, value)),
                _ => invalid_environment = true,
            }
        }
        Self {
            environment,
            invalid_environment,
            extra: Mutex::new(Vec::new()),
        }
    }
    pub fn stream(&self) -> Result<Masker, CoreError> {
        if self.invalid_environment {
            return Err(error("masking_unavailable", "invalid masking policy"));
        }
        let env: Vec<_> = self
            .environment
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let extra = self.extra.lock().unwrap();
        let extra: Vec<_> = extra
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        Masker::new_fixed_width(&env, &extra)
            .map_err(|_| error("masking_unavailable", "invalid masking policy"))
    }
    pub fn add_literal(&self, label: &str, value: &str) -> Result<(), CoreError> {
        if self.invalid_environment {
            return Err(error("masking_unavailable", "invalid masking policy"));
        }
        let env: Vec<_> = self
            .environment
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let mut extra = self.extra.lock().unwrap();
        let mut candidate: Vec<_> = extra
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        candidate.push((label, value));
        Masker::new_fixed_width(&env, &candidate)
            .map_err(|_| error("masking_unavailable", "invalid masking policy"))?;
        extra.push((label.into(), value.into()));
        Ok(())
    }
    /// Mask complete logical context before callers paginate or select a range.
    /// Undecided active suffixes are unavailable, never treated as end-of-stream.
    pub fn bytes(&self, bytes: &[u8], complete: bool) -> Result<Vec<u8>, CoreError> {
        let mut stream = self.stream()?;
        let mut safe = stream
            .push(bytes)
            .map_err(|_| error("masking_unavailable", "masking failed"))?;
        if complete {
            safe.extend(
                stream
                    .finish()
                    .map_err(|_| error("masking_unavailable", "masking failed"))?,
            );
        }
        Ok(safe)
    }
    pub fn text(&self, text: &str, complete: bool) -> Result<String, CoreError> {
        Ok(String::from_utf8_lossy(&self.bytes(text.as_bytes(), complete)?).into_owned())
    }
    pub fn block(
        &self,
        block: &mut crate::terminal::Block,
        complete: bool,
    ) -> Result<(), CoreError> {
        block.output = self.text(&block.output, complete)?;
        if let Some(command) = &mut block.command {
            *command = self.text(command, true)?;
        }
        Ok(())
    }
    pub fn value(&self, value: &Value) -> Result<Value, CoreError> {
        Ok(match value {
            Value::String(s) => Value::String(self.text(s, true)?),
            Value::Array(a) => {
                Value::Array(a.iter().map(|v| self.value(v)).collect::<Result<_, _>>()?)
            }
            Value::Object(o) => {
                let mut out = serde_json::Map::new();
                for (key, value) in o {
                    let lower = key.to_ascii_lowercase();
                    if lower.contains("token")
                        || lower.contains("capability")
                        || lower.contains("auth")
                    {
                        continue;
                    }
                    // Input payloads must be decoded before masking, not scanned as base64.
                    let value = if key == "data" {
                        if let Some(encoded) = value.as_str() {
                            match base64::engine::general_purpose::STANDARD.decode(encoded) {
                                Ok(mut bytes) => {
                                    let safe = self.bytes(&bytes, true);
                                    bytes.fill(0);
                                    std::hint::black_box(&bytes);
                                    json!(base64::engine::general_purpose::STANDARD.encode(safe?))
                                }
                                Err(_) => self.value(value)?,
                            }
                        } else {
                            self.value(value)?
                        }
                    } else {
                        self.value(value)?
                    };
                    out.insert(self.text(key, true)?, value);
                }
                Value::Object(out)
            }
            _ => value.clone(),
        })
    }
}
impl Drop for Policy {
    fn drop(&mut self) {
        for (_, value) in self
            .environment
            .iter_mut()
            .chain(self.extra.get_mut().unwrap().iter_mut())
        {
            // Best effort for the captured copy; matcher storage wipes itself.
            let mut bytes = std::mem::take(value).into_bytes();
            bytes.fill(0);
            std::hint::black_box(&bytes);
        }
    }
}
