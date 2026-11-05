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
    /// Only authenticated owner snapshots may show unconfirmed prefixes.
    pub(super) fn display_bytes(&self, bytes: &[u8]) -> Result<Vec<u8>, CoreError> {
        if self.invalid_environment { return Err(error("masking_unavailable", "invalid masking policy")); }
        let env: Vec<_> = self.environment.iter().map(|(k,v)| (k.as_str(),v.as_str())).collect();
        let extra = self.extra.lock().unwrap();
        let extra: Vec<_> = extra.iter().map(|(k,v)| (k.as_str(),v.as_str())).collect();
        Masker::mask_display(&env, &extra, bytes)
            .map_err(|_| error("masking_unavailable", "masking failed"))
    }
    pub fn text(&self, text: &str, complete: bool) -> Result<String, CoreError> {
        Ok(String::from_utf8_lossy(&self.bytes(text.as_bytes(), complete)?).into_owned())
    }
    /// Strict screen/history/search context, masked before pagination or selection.
    pub fn terminal_lines(&self, terminal: &crate::terminal::Terminal) -> Result<Vec<String>, CoreError> {
        self.terminal_lines_mode(terminal, false)
    }
    /// Owner live display only; history/search/copy/agent projections stay strict.
    pub fn display_lines(&self, terminal: &crate::terminal::Terminal) -> Result<Vec<String>, CoreError> {
        self.terminal_lines_mode(terminal, true)
    }
    fn terminal_lines_mode(&self, terminal: &crate::terminal::Terminal, display: bool) -> Result<Vec<String>, CoreError> {
        let (_, _, mut lines, wraps, truncated) = terminal.text_context();
        if display { self.mask_display_lines(&mut lines, &wraps, terminal.text_cursor())?; }
        else { self.mask_lines(&mut lines, &wraps, terminal.text_cursor())?; }
        if truncated {
            for (line, wrapped) in lines.iter_mut().zip(&wraps) {
                *line = "*".repeat(line.len());
                if !wrapped { break; }
            }
        }
        Ok(lines)
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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_claude_stays_readable() {
        // Synthetic owner capability, never a machine credential. Its prefix
        // overlaps ordinary typed text; neither Claud nor Claude is a secret.
        let policy = Policy { environment: Vec::new(), invalid_environment: false, extra: Mutex::new(Vec::new()) };
        policy.add_literal("CLANKY_OWNER_TOKEN", &format!("de{}", "0".repeat(62))).unwrap();
        let mut terminal = crate::terminal::Terminal::new(2, 80);
        for expected in ["C", "Cl", "Cla", "Clau", "Claud", "Claude"] {
            terminal.feed("bash", &expected.as_bytes()[expected.len()-1..]);
            let visible = policy.display_lines(&terminal).unwrap();
            assert_eq!(visible[0].trim_end(), expected);
        }
        assert_eq!(policy.display_lines(&terminal).unwrap()[0].trim_end(), "Claude");
        assert_eq!(policy.terminal_lines(&terminal).unwrap()[0].trim_end(), "Clau**");
        assert_eq!(policy.bytes(b"Claude", false).unwrap(), b"Clau");
        // Header-complete values are hidden even below the agent trigger length.
        let mut token = crate::terminal::Terminal::new(2, 80);
        token.feed("bash", b"sk-A");
        assert!(policy.display_lines(&token).unwrap()[0].trim_end() == "****");
        assert!(policy.bytes(b"sk-A", false).unwrap().is_empty());
        let mut known = crate::terminal::Terminal::new(2, 80);
        let generated = format!("de{}", "0".repeat(62));
        known.feed("bash", generated.as_bytes());
        assert!(policy.display_lines(&known).unwrap()[0].trim_end() == "*".repeat(64));
        assert!(policy.terminal_lines(&known).unwrap()[0].trim_end() == "*".repeat(64));
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
