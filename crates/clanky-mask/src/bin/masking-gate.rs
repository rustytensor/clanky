//! Independent fixed-expectation gate. Synthetic credentials never leave memory.
use clanky_mask::{Error, Masker};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{Ordering, compiler_fence};
use std::time::{SystemTime, UNIX_EPOCH};

type Check<T = ()> = Result<T, &'static str>;

// No Debug: even assertion/panic formatting must not expose synthetic inputs.
struct Secret(Vec<u8>);
impl Secret {
    fn from(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
    fn text(&self) -> Check<&str> {
        std::str::from_utf8(&self.0).map_err(|_| "FAIL: generator encoding")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        for byte in self.0.iter_mut() {
            // The allocation is exclusively borrowed and every pointer is valid.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}
fn join(parts: &[&[u8]]) -> Secret {
    let mut out = vec![0; parts.iter().map(|p| p.len()).sum()];
    let mut at = 0;
    for p in parts {
        out[at..at + p.len()].copy_from_slice(p);
        at += p.len();
    }
    Secret::from(out)
}
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}
struct Rng(u64);
impl Rng {
    fn bytes(&mut self, alphabet: &[u8], n: usize) -> Secret {
        let mut bytes = vec![0; n];
        for b in &mut bytes {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            *b = alphabet[(self.0 as usize) % alphabet.len()];
        }
        Secret::from(bytes)
    }
}
struct Case {
    input: Secret,
    marker: &'static [u8],
}
struct Data {
    cases: Vec<Case>,
    bodies: Vec<Secret>,
    unicode: Secret,
    literals: Vec<Secret>,
    long_body: Secret,
    huge: Secret,
}
const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
impl Data {
    fn new(seed: u64) -> Self {
        let mut rng = Rng(seed.max(1));
        let mut cases = Vec::new();
        for (prefix, count, alphabet, marker) in [
            (&b"sk-"[..], 48, ALNUM, &b"[masked:OPENAI]"[..]),
            (
                b"sk-or-v1-",
                64,
                &b"0123456789abcdef"[..],
                b"[masked:OPENAI]",
            ),
            (b"ghp_", 36, ALNUM, b"[masked:GITHUB]"),
            (
                b"github_pat_",
                82,
                &b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_"[..],
                b"[masked:GITHUB]",
            ),
            (
                b"AKIA",
                16,
                &b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[..],
                b"[masked:AWS]",
            ),
            (b"xoxb-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxp-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxa-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxr-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxs-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxc-", 32, ALNUM, b"[masked:SLACK]"),
            (b"xoxe-", 32, ALNUM, b"[masked:SLACK]"),
            (b"hf_", 34, ALNUM, b"[masked:HUGGINGFACE]"),
            (b"rpa_", 48, ALNUM, b"[masked:RUNPOD]"),
            (
                b"Authorization: Bearer ",
                40,
                &b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~+/="[..],
                b"[masked:BEARER]",
            ),
        ] {
            let tail = rng.bytes(alphabet, count);
            cases.push(Case {
                input: join(&[prefix, &tail.0]),
                marker,
            });
        }
        let mut bodies = Vec::new();
        for kind in [
            "PRIVATE KEY",
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "DSA PRIVATE KEY",
            "OPENSSH PRIVATE KEY",
            "ENCRYPTED PRIVATE KEY",
        ] {
            let body = rng.bytes(
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
                96,
            );
            let begin = format!("-----BEGIN {kind}-----\n");
            let end = format!("\n-----END {kind}-----");
            cases.push(Case {
                input: join(&[begin.as_bytes(), &body.0, end.as_bytes()]),
                marker: b"[masked:PRIVATE_KEY]",
            });
            bodies.push(body);
        }
        cases.push(Case {
            input: rng.bytes(ALNUM, 32),
            marker: b"[masked:TEST_API_TOKEN]",
        });
        cases.push(Case {
            input: rng.bytes(ALNUM, 24),
            marker: b"[masked:EXTRA]",
        });
        // Twelve distinct Unicode scalar values, generated at runtime.
        let mut unicode = vec![0; 36];
        for (i, chunk) in unicode.chunks_exact_mut(3).enumerate() {
            let scalar = char::from_u32(0x4e00 + ((seed as u32).wrapping_add(i as u32) % 1000))
                .unwrap_or('雪');
            scalar.encode_utf8(chunk);
        }
        let literals = (0..129).map(|_| rng.bytes(ALNUM, 24)).collect();
        let long_body = rng.bytes(ALNUM, 131_072);
        let huge = rng.bytes(ALNUM, 65_537);
        Self {
            cases,
            bodies,
            unicode: Secret::from(unicode),
            literals,
            long_body,
            huge,
        }
    }
    fn env(&self) -> Check<[(&str, &str); 1]> {
        Ok([("TEST_API_TOKEN", self.cases[21].input.text()?)])
    }
    fn extra(&self) -> Check<[(&str, &str); 1]> {
        Ok([("EXTRA", self.cases[22].input.text()?)])
    }
    fn safe(&self, bytes: &[u8]) -> Check {
        if self.cases.iter().any(|c| contains(bytes, &c.input.0))
            || self.bodies.iter().any(|b| contains(bytes, &b.0))
            || contains(bytes, &self.unicode.0)
            || contains(bytes, &self.long_body.0)
            || contains(bytes, &self.huge.0)
            || self
                .literals
                .iter()
                .any(|literal| contains(bytes, &literal.0))
        {
            Err("FAIL: secret detected")
        } else {
            Ok(())
        }
    }
    fn error(&self, e: Error) -> Check {
        let display = Secret::from(format!("{e}").into_bytes());
        let debug = Secret::from(format!("{e:?}").into_bytes());
        self.safe(&display.0)?;
        self.safe(&debug.0)
    }
    fn value<T>(&self, result: Result<T, Error>) -> Check<T> {
        match result {
            Ok(v) => Ok(v),
            Err(e) => {
                self.error(e)?;
                Err("FAIL: unexpected API error")
            }
        }
    }
    fn rejected<T>(&self, result: Result<T, Error>) -> Check {
        match result {
            Ok(_) => Err("FAIL: invalid operation accepted"),
            Err(e) => self.error(e),
        }
    }
    fn masker(&self) -> Check<Masker> {
        let masker = self.value(Masker::new(&self.env()?, &self.extra()?))?;
        // Longest trigger is the 37-byte ENCRYPTED PRIVATE KEY header;
        // configured literals here are 32 and 24 bytes. Only 36 may be held.
        if masker.hold_limit() != 36 {
            return Err("FAIL: hold limit value");
        }
        Ok(masker)
    }
    fn bound(&self, m: &Masker) -> Check {
        if m.held_bytes() > m.hold_limit() {
            Err("FAIL: hold bound")
        } else {
            Ok(())
        }
    }
    fn chunks(&self, mut m: Masker, input: &[u8], cuts: &[usize], expected: &[u8]) -> Check {
        let mut output = join(&[]);
        let mut at = 0;
        for end in cuts.iter().copied().chain(std::iter::once(input.len())) {
            let fragment = Secret::from(self.value(m.push(&input[at..end]))?);
            self.bound(&m)?;
            output = join(&[&output.0, &fragment.0]);
            self.safe(&output.0)?;
            at = end;
        }
        let fragment = Secret::from(self.value(m.finish())?);
        self.bound(&m)?;
        output = join(&[&output.0, &fragment.0]);
        self.safe(&output.0)?;
        if output.0.as_slice() != expected {
            return Err("FAIL: fixed output mismatch");
        }
        Ok(())
    }
    fn split_fault(&self, input: &[u8]) -> Check {
        let mut output = join(&[]);
        for chunk in [&input[..2], &input[2..]] {
            let mut m = self.masker()?;
            let pushed = Secret::from(self.value(m.push(chunk))?);
            let ended = Secret::from(self.value(m.finish())?);
            output = join(&[&output.0, &pushed.0, &ended.0]);
        }
        // The deliberate broken stream must leak this first sk token. Never print it.
        if self.safe(&output.0).is_err() && output.0.as_slice() == input {
            Err("FAIL: split boundary 2")
        } else {
            Err("FAIL: split fault not demonstrated")
        }
    }
    fn run(&self, fault: bool) -> Check {
        for (index, case) in self.cases.iter().enumerate() {
            let input = &case.input.0;
            // The owner-only snapshot mode must still hide each complete
            // generated credential, exactly one star per raw byte.
            let display = Secret::from(self.value(Masker::mask_display(&self.env()?, &self.extra()?, input))?);
            self.safe(&display.0)?;
            if display.0.len() != input.len() || display.0.iter().any(|b| *b != b'*') {
                return Err("FAIL: owner display exposed a complete secret");
            }
            let wrapped = join(&[b"before ", input, b" after"]);
            let expected = join(&[b"before ", case.marker, b" after"]);
            self.chunks(self.masker()?, &wrapped.0, &[], &expected.0)?;
            for cut in 0..=input.len() {
                if fault && index == 0 && cut == 2 {
                    return self.split_fault(input);
                }
                self.chunks(self.masker()?, input, &[cut], case.marker)?;
            }
            self.chunks(self.masker()?, input, &[2, input.len() - 2], case.marker)?;
            let cuts: Vec<usize> = (1..input.len()).collect();
            self.chunks(self.masker()?, input, &cuts, case.marker)?;
            let utf = join(&["雪🙂 ".as_bytes(), input, " café\r\n".as_bytes()]);
            let expected = join(&["雪🙂 ".as_bytes(), case.marker, " café\r\n".as_bytes()]);
            let cuts: Vec<usize> = (1..utf.0.len()).collect();
            self.chunks(self.masker()?, &utf.0, &cuts, &expected.0)?;
        }
        let adjacent = join(&[&self.cases[21].input.0, &self.cases[22].input.0]);
        self.chunks(
            self.masker()?,
            &adjacent.0,
            &[31, 33],
            b"[masked:TEST_API_TOKEN][masked:EXTRA]",
        )?;
        let aws = join(&[&self.cases[4].input.0, &self.cases[4].input.0]);
        self.chunks(
            self.masker()?,
            &aws.0,
            &[19, 21],
            b"[masked:AWS][masked:AWS]",
        )?;
        let variable = join(&[&self.cases[0].input.0, &self.cases[0].input.0]);
        self.chunks(self.masker()?, &variable.0, &[51], b"[masked:OPENAI]")?;
        let short = b"sk-abcde ghp_short hf_short rpa_short AKIA123";
        self.chunks(self.masker()?, short, &[2, 17], short)?;
        self.chunks(
            self.masker()?,
            b"\xff\xfe\x80\0!",
            &[1, 3],
            b"\xff\xfe\x80\0!",
        )?;
        self.configuration()?;
        self.streaming()
    }
    fn configuration(&self) -> Check {
        let ignored = "ordinary nonsecret text";
        let short = self.unicode.text()?.chars().take(11).collect::<String>();
        let short = Secret::from(short.into_bytes());
        let m = self.value(Masker::new(
            &[("IGNORE_THIS", ignored), ("SHORT_TOKEN", short.text()?)],
            &[],
        ))?;
        let input = join(&[ignored.as_bytes(), b" ", &short.0]);
        // These are not credentials; ignored names and short values pass unchanged.
        self.ignored(m, &input.0)?;
        let m = self.value(Masker::new(&[("mixed_tOkEn", self.unicode.text()?)], &[]))?;
        self.chunks(m, &self.unicode.0, &[1, 17], b"[masked:mixed_tOkEn]")?;
        self.rejected(Masker::new(&[], &[("EXTRA", "")]))?;
        for label in ["", "1BAD", "BAD-NAME"] {
            self.rejected(Masker::new(&[], &[(label, ignored)]))?;
        }
        let label = "L".repeat(65);
        self.rejected(Masker::new(&[], &[(label.as_str(), ignored)]))?;
        self.rejected(Masker::new(&[], &[("EXTRA", self.huge.text()?)]))?;
        self.rejected(Masker::new(&[("HUGE_TOKEN", self.huge.text()?)], &[]))?;
        let refs: Vec<(&str, &str)> = self
            .literals
            .iter()
            .map(|v| Ok(("EXTRA", v.text()?)))
            .collect::<Check<_>>()?;
        self.rejected(Masker::new(&[], &refs))?;
        let common = self.cases[21].input.text()?;
        let longer = join(&[common.as_bytes(), b"!"]);
        let m = self.value(Masker::new(
            &[],
            &[("SHORT", common), ("LONG", longer.text()?)],
        ))?;
        self.chunks(m, &longer.0, &[common.len()], b"[masked:LONG]")?;
        let failed = join(&[common.as_bytes(), b"?"]);
        let m = self.value(Masker::new(
            &[],
            &[("SHORT", common), ("LONG", longer.text()?)],
        ))?;
        self.chunks(m, &failed.0, &[common.len()], b"[masked:SHORT]?")?;
        let m = self.value(Masker::new(
            &[("FIRST_TOKEN", common)],
            &[("SECOND", common)],
        ))?;
        self.chunks(m, common.as_bytes(), &[], b"[masked:FIRST_TOKEN]")?;
        // A secret embedded in either a label or its complete marker is unsafe.
        let unsafe_label = join(&[b"PREFIX_", common.as_bytes()]);
        self.rejected(Masker::new(&[], &[(unsafe_label.text()?, common)]))?;
        self.rejected(Masker::new(&[], &[("EXTRA", "masked")]))?;
        let label = self.cases[2].input.text()?;
        self.rejected(Masker::new(&[], &[(label, ignored)]))?;
        Ok(())
    }
    fn ignored(&self, mut m: Masker, input: &[u8]) -> Check {
        let a = Secret::from(self.value(m.push(input))?);
        let b = Secret::from(self.value(m.finish())?);
        let output = join(&[&a.0, &b.0]);
        self.safe(&output.0)?;
        if output.0.as_slice() != input {
            return Err("FAIL: ignored value changed");
        }
        Ok(())
    }
    fn streaming(&self) -> Check {
        let mut m = self.masker()?;
        let out = Secret::from(self.value(m.push(b"!"))?);
        self.safe(&out.0)?;
        if out.0.as_slice() != b"!" || m.held_bytes() != 0 {
            return Err("FAIL: ordinary byte held");
        }
        let out = Secret::from(self.value(m.finish())?);
        self.safe(&out.0)?;
        if !out.0.is_empty() {
            return Err("FAIL: unexpected finish bytes");
        }
        self.rejected(m.push(b"!"))?;
        self.rejected(m.finish())?;
        self.chunks(self.masker()?, b"sk-", &[1], b"sk-")?;
        for case in &self.cases[15..21] {
            let cut = case.input.0.len()
                - (case
                    .input
                    .0
                    .iter()
                    .rev()
                    .position(|b| *b == b'\n')
                    .unwrap_or(0)
                    + 1);
            self.chunks(
                self.masker()?,
                &case.input.0[..cut],
                &[2],
                b"[masked:PRIVATE_KEY]",
            )?;
        }
        // Bodies greatly exceed hold_limit; check after each individual byte and chunk.
        for (prefix, suffix, marker) in [
            (&b"sk-"[..], &b" "[..], &b"[masked:OPENAI] "[..]),
            (
                // Assemble public PEM delimiters so secret scans do not mistake them for keys.
                concat!("-----BEGIN ", "PRIVATE KEY-----\n").as_bytes(),
                concat!("\n-----END ", "PRIVATE KEY-----!").as_bytes(),
                b"[masked:PRIVATE_KEY]!",
            ),
        ] {
            let body = &self.long_body;
            let input = join(&[prefix, &body.0, suffix]);
            let mut m = self.masker()?;
            let mut output = join(&[]);
            for chunk in input.0.chunks(1) {
                let next = Secret::from(self.value(m.push(chunk))?);
                self.bound(&m)?;
                output = join(&[&output.0, &next.0]);
                self.safe(&output.0)?;
                if contains(&output.0, &body.0) {
                    return Err("FAIL: long body leak");
                }
            }
            let next = Secret::from(self.value(m.finish())?);
            output = join(&[&output.0, &next.0]);
            self.safe(&output.0)?;
            if output.0.as_slice() != marker {
                return Err("FAIL: long body output");
            }
            self.chunks(self.masker()?, &input.0, &[], marker)?;
        }
        Ok(())
    }
}

fn child() -> Check {
    let mut seed = [0u8; 8];
    std::io::stdin()
        .read_exact(&mut seed)
        .map_err(|_| "FAIL: seed input")?;
    let data = Data::new(u64::from_le_bytes(seed));
    let fault = match std::env::var("CLANKY_GATE_FAULT") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(s) if s.is_empty() => false,
        Ok(s) if s == "split" => true,
        _ => return Err("FAIL: unknown fault"),
    };
    data.run(fault)
}
fn parent() -> Check {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "FAIL: clock")?
        .as_nanos();
    let seed = ((nanos as u64) ^ ((nanos >> 64) as u64) ^ u64::from(std::process::id())).max(1);
    let data = Data::new(seed);
    let mut cmd = Command::new(std::env::current_exe().map_err(|_| "FAIL: executable")?);
    cmd.arg("--child")
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let fault = match std::env::var("CLANKY_GATE_FAULT") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(s) if s.is_empty() => false,
        Ok(s) if s == "split" => {
            cmd.env("CLANKY_GATE_FAULT", "split");
            true
        }
        _ => return Err("FAIL: unknown fault"),
    };
    let mut process = cmd.spawn().map_err(|_| "FAIL: child launch")?;
    let wrote = match process.stdin.take() {
        Some(mut stdin) => stdin.write_all(&seed.to_le_bytes()).is_ok(),
        None => false,
    };
    let captured = process
        .wait_with_output()
        .map_err(|_| "FAIL: child capture")?;
    let stdout = Secret::from(captured.stdout);
    let stderr = Secret::from(captured.stderr);
    data.safe(&stdout.0)?;
    data.safe(&stderr.0)?;
    // Also catch a credential divided between output streams.
    let combined = join(&[&stdout.0, &stderr.0]);
    data.safe(&combined.0)?;
    if !wrote {
        return Err("FAIL: child input");
    }
    if fault
        && stdout.0.as_slice() == b"FAIL: split boundary 2\n"
        && stderr.0.is_empty()
        && !captured.status.success()
    {
        return Err("FAIL: split boundary 2");
    }
    if fault {
        return Err("FAIL: split fault not demonstrated");
    }
    if !captured.status.success() || stdout.0.as_slice() != b"PASS\n" || !stderr.0.is_empty() {
        return Err("FAIL: masking child");
    }
    Ok(())
}
fn main() {
    let is_child = std::env::args().nth(1).as_deref() == Some("--child");
    let result = if is_child { child() } else { parent() };
    match result {
        Ok(()) => println!("PASS"),
        Err(message) => {
            println!("{message}");
            std::process::exit(1);
        }
    }
}
