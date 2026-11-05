//! Incremental byte-stream masking, independent of terminal or environment I/O.
//! Caller-owned inputs and configuration are not erased by this crate.
mod secret;

use secret::{Secret, wipe};
use std::fmt;

/// Errors deliberately carry no caller data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidLabel,
    EmptyLiteral,
    PatternTooLong,
    TooManyPatterns,
    UnsafeMarker,
    Finished,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLabel => "invalid masking label",
            Self::EmptyLiteral => "empty masking literal",
            Self::PatternTooLong => "masking literal exceeds limit",
            Self::TooManyPatterns => "too many masking literals",
            Self::UnsafeMarker => "unsafe masking marker",
            Self::Finished => "masking stream already finished",
        })
    }
}
impl std::error::Error for Error {}

struct Literal {
    value: Secret,
    marker: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Alphabet {
    Token,
    Bearer,
    Aws,
}
impl Alphabet {
    fn contains(self, byte: u8) -> bool {
        byte.is_ascii_alphanumeric()
            || match self {
                Self::Token => matches!(byte, b'_' | b'-'),
                Self::Bearer => matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'='),
                Self::Aws => false,
            }
    }
}

#[derive(Clone, Copy)]
enum Tail {
    None,
    Run(Alphabet),
    Private { end: &'static [u8], position: usize },
}

struct Builtin {
    prefix: &'static [u8],
    body: usize,
    alphabet: Alphabet,
    insensitive: bool,
    marker: &'static [u8],
    tail: Tail,
}

const TOKEN: Alphabet = Alphabet::Token;
const BUILTINS: &[Builtin] = &[
    Builtin {
        prefix: b"sk-",
        body: 6,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:OPENAI]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"ghp_",
        body: 12,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:GITHUB]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"github_pat_",
        body: 12,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:GITHUB]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"hf_",
        body: 12,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:HUGGINGFACE]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"rpa_",
        body: 12,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:RUNPOD]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"xoxb-",
        body: 12,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:SLACK]",
        tail: Tail::Run(TOKEN),
    },
    Builtin {
        prefix: b"AKIA",
        body: 16,
        alphabet: Alphabet::Aws,
        insensitive: false,
        marker: b"[masked:AWS]",
        tail: Tail::None,
    },
    Builtin {
        prefix: b"Authorization: Bearer ",
        body: 1,
        alphabet: Alphabet::Bearer,
        insensitive: true,
        marker: b"[masked:BEARER]",
        tail: Tail::Run(Alphabet::Bearer),
    },
    // Assemble public PEM delimiters so secret scans do not mistake them for keys.
    private(
        concat!("-----BEGIN ", "PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "PRIVATE KEY-----").as_bytes(),
    ),
    private(
        concat!("-----BEGIN ", "RSA PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "RSA PRIVATE KEY-----").as_bytes(),
    ),
    private(
        concat!("-----BEGIN ", "EC PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "EC PRIVATE KEY-----").as_bytes(),
    ),
    private(
        concat!("-----BEGIN ", "DSA PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "DSA PRIVATE KEY-----").as_bytes(),
    ),
    private(
        concat!("-----BEGIN ", "OPENSSH PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "OPENSSH PRIVATE KEY-----").as_bytes(),
    ),
    private(
        concat!("-----BEGIN ", "ENCRYPTED PRIVATE KEY-----").as_bytes(),
        concat!("-----END ", "ENCRYPTED PRIVATE KEY-----").as_bytes(),
    ),
];

const fn private(prefix: &'static [u8], end: &'static [u8]) -> Builtin {
    Builtin {
        prefix,
        body: 0,
        alphabet: TOKEN,
        insensitive: false,
        marker: b"[masked:PRIVATE_KEY]",
        tail: Tail::Private { end, position: 0 },
    }
}

impl Builtin {
    fn len(&self) -> usize {
        self.prefix.len() + self.body
    }

    // A successful comparison means either a pending prefix or a complete
    // recognition trigger. Bytes past the trigger are deliberately irrelevant.
    fn matches(&self, bytes: &[u8]) -> bool {
        bytes.iter().take(self.len()).enumerate().all(|(i, &byte)| {
            if i < self.prefix.len() {
                // Slack accepts any ASCII letter in the fourth prefix byte.
                if self.prefix == b"xoxb-" && i == 3 {
                    byte.is_ascii_alphabetic()
                } else if self.insensitive {
                    byte.eq_ignore_ascii_case(&self.prefix[i])
                } else {
                    byte == self.prefix[i]
                }
            } else if matches!(self.alphabet, Alphabet::Aws) {
                byte.is_ascii_uppercase() || byte.is_ascii_digit()
            } else {
                self.alphabet.contains(byte)
            }
        })
    }
}

/// One ordered stream. No Debug implementation is provided for secret storage.
pub struct Masker {
    literals: Vec<Literal>,
    hold: Secret,
    held: usize,
    tail: Tail,
    finished: bool,
    fixed_width: bool,
    /// Bytes that can begin a literal or a built-in trigger. Any other byte, met
    /// with nothing held and no open tail, is released unchanged by drain().
    starts: [bool; 256],
}

impl Masker {
    /// Copy eligible environment values and named extra literals into wiped storage.
    /// Does not read the process environment. Invalid configuration fails closed;
    /// caller-owned input copies remain the caller's responsibility to protect.
    pub fn new(env: &[(&str, &str)], extra: &[(&str, &str)]) -> Result<Self, Error> {
        Self::configured(env, extra, false)
    }

    /// Mask recognized bytes with one ASCII star per raw byte instead of labels.
    /// Uses the same matching and stream withholding rules as `new`. Literals
    /// containing stars are rejected: replacement plus adjacent text must not
    /// manufacture another configured secret across a redaction boundary.
    /// Errors never include configuration values; callers must fail closed.
    pub fn new_fixed_width(env: &[(&str, &str)], extra: &[(&str, &str)]) -> Result<Self, Error> {
        Self::configured(env, extra, true)
    }

    fn configured(
        env: &[(&str, &str)],
        extra: &[(&str, &str)],
        fixed_width: bool,
    ) -> Result<Self, Error> {
        let mut literals: Vec<Literal> = Vec::new();
        for &(name, value) in env
            .iter()
            .filter(|(name, value)| environment_name(name) && value.chars().count() >= 12)
            .chain(extra.iter())
        {
            if !valid_label(name.as_bytes()) {
                return Err(Error::InvalidLabel);
            }
            if value.is_empty() {
                return Err(Error::EmptyLiteral);
            }
            if value.len() > 65_536 {
                return Err(Error::PatternTooLong);
            }
            if fixed_width && value.as_bytes().contains(&b'*') {
                return Err(Error::UnsafeMarker);
            }
            if literals
                .iter()
                .any(|literal| literal.value.bytes() == value.as_bytes())
            {
                continue;
            }
            if literals.len() == 128 {
                return Err(Error::TooManyPatterns);
            }
            // Allocate each secret exactly once, without growth or cloning.
            literals.push(Literal {
                value: Secret::copy(value.as_bytes()),
                marker: Vec::new(),
            });
        }

        if !fixed_width {
            // Validate every effective label, including aliases of duplicate values.
            // Temporary markers use wiped storage until validation establishes that
            // they do not contain configured secrets or built-in recognition triggers.
            for &(name, value) in env
                .iter()
                .filter(|(name, value)| environment_name(name) && value.chars().count() >= 12)
                .chain(extra.iter())
            {
                let label = name.as_bytes();
                if unsafe_output(label, &literals) {
                    return Err(Error::UnsafeMarker);
                }
                let mut marker = Secret::zeroed(8 + label.len() + 1);
                marker.bytes_mut()[..8].copy_from_slice(b"[masked:");
                marker.bytes_mut()[8..8 + label.len()].copy_from_slice(label);
                marker.bytes_mut()[8 + label.len()] = b']';
                if unsafe_output(marker.bytes(), &literals) {
                    return Err(Error::UnsafeMarker);
                }
                if let Some(literal) = literals
                    .iter_mut()
                    .find(|literal| literal.value.bytes() == value.as_bytes())
                {
                    if literal.marker.is_empty() {
                        literal.marker = marker.bytes().to_vec();
                    }
                }
            }
            for builtin in BUILTINS {
                let label = &builtin.marker[8..builtin.marker.len() - 1];
                if unsafe_output(label, &literals) || unsafe_output(builtin.marker, &literals) {
                    return Err(Error::UnsafeMarker);
                }
            }
        }
        let limit = literals
            .iter()
            .map(|literal| literal.value.bytes().len())
            .chain(BUILTINS.iter().map(Builtin::len))
            .max()
            .unwrap_or(1);
        let mut starts = [false; 256];
        for (byte, start) in starts.iter_mut().enumerate() {
            let byte = byte as u8;
            *start = literals.iter().any(|literal| literal.value.bytes()[0] == byte)
                || BUILTINS.iter().any(|builtin| builtin.matches(&[byte]));
        }
        Ok(Self {
            starts,
            literals,
            hold: Secret::zeroed(limit),
            held: 0,
            tail: Tail::None,
            finished: false,
            fixed_width,
        })
    }

    /// Number of undecided bytes withheld from the agent at this moment.
    pub fn held_bytes(&self) -> usize {
        self.held
    }
    /// Maximum undecided suffix after a push: longest recognition trigger minus one.
    pub fn hold_limit(&self) -> usize {
        self.hold.bytes().len() - 1
    }

    /// Return only bytes safe to release; retain an undecided suffix across calls.
    /// Each command/read stream needs its own instance and ordered input chunks.
    pub fn push(&mut self, input: &[u8]) -> Result<Vec<u8>, Error> {
        if self.finished {
            return Err(Error::Finished);
        }
        let mut output = Vec::with_capacity(input.len());
        let mut index = 0;
        while index < input.len() {
            // Fast path, same result as drain(): with nothing held and no open tail,
            // a byte that cannot begin any trigger is released as it is.
            if self.held == 0 && matches!(self.tail, Tail::None) {
                let run = input[index..]
                    .iter()
                    .position(|byte| self.starts[usize::from(*byte)])
                    .unwrap_or(input.len() - index);
                if run > 0 {
                    output.extend_from_slice(&input[index..index + run]);
                    index += run;
                    continue;
                }
            }
            // drain() always leaves fewer bytes than the buffer capacity. The fixed
            // allocation never grows, even for arbitrarily long input chunks.
            self.hold.bytes_mut()[self.held] = input[index];
            self.held += 1;
            self.drain(false, &mut output);
            index += 1;
        }
        Ok(output)
    }

    /// End the stream, flushing nonmatches and suppressing recognized secret tails.
    /// Calling finish or push again returns a static Finished error.
    pub fn finish(&mut self) -> Result<Vec<u8>, Error> {
        if self.finished {
            return Err(Error::Finished);
        }
        self.finished = true;
        let mut output = Vec::new();
        self.drain(true, &mut output);
        self.tail = Tail::None;
        Ok(output)
    }

    fn consume(&mut self, count: usize) {
        let remaining = self.held - count;
        self.hold.bytes_mut().copy_within(count..self.held, 0);
        wipe(&mut self.hold.bytes_mut()[remaining..self.held]);
        self.held = remaining;
    }

    fn drain(&mut self, eof: bool, output: &mut Vec<u8>) {
        while self.held != 0 {
            let byte = self.hold.bytes()[0];
            match self.tail {
                Tail::Run(alphabet) => {
                    if alphabet.contains(byte) {
                        if self.fixed_width {
                            output.push(b'*');
                        }
                        self.consume(1);
                        continue;
                    }
                    self.tail = Tail::None;
                }
                Tail::Private { end, mut position } => {
                    // Only public delimiter positions are retained, never body
                    // bytes. Prefix fallback handles overlapping hyphen runs.
                    while position > 0 && end[position] != byte {
                        position = public_fallback(end, position);
                    }
                    if end[position] == byte {
                        position += 1;
                    }
                    self.tail = if position == end.len() {
                        Tail::None
                    } else {
                        Tail::Private { end, position }
                    };
                    if self.fixed_width {
                        output.push(b'*');
                    }
                    self.consume(1);
                    continue;
                }
                Tail::None => {}
            }
            let bytes = &self.hold.bytes()[..self.held];
            let mut pending = false;
            let mut best_len = 0;
            let mut best_literal = None;
            let mut best_builtin = None;
            for (i, literal) in self.literals.iter().enumerate() {
                let value = literal.value.bytes();
                let common = bytes.len().min(value.len());
                if bytes[..common] == value[..common] {
                    if bytes.len() < value.len() {
                        pending = true;
                    } else if value.len() > best_len {
                        best_len = value.len();
                        best_literal = Some(i);
                        best_builtin = None;
                    }
                }
            }
            for (i, builtin) in BUILTINS.iter().enumerate() {
                if builtin.matches(bytes) {
                    if bytes.len() < builtin.len() {
                        pending = true;
                    } else if builtin.len() > best_len {
                        best_len = builtin.len();
                        best_builtin = Some(i);
                        best_literal = None;
                    }
                }
            }
            if pending && !eof {
                return;
            }
            if let Some(i) = best_literal {
                if self.fixed_width {
                    output.resize(output.len() + best_len, b'*');
                } else {
                    output.extend_from_slice(&self.literals[i].marker);
                }
                self.consume(best_len);
            } else if let Some(i) = best_builtin {
                if self.fixed_width {
                    output.resize(output.len() + best_len, b'*');
                } else {
                    output.extend_from_slice(BUILTINS[i].marker);
                }
                self.tail = BUILTINS[i].tail;
                self.consume(best_len);
            } else {
                output.push(byte);
                self.consume(1);
            }
        }
    }
}

fn valid_label(label: &[u8]) -> bool {
    !label.is_empty()
        && label.len() <= 64
        && (label[0].is_ascii_alphabetic() || label[0] == b'_')
        && label
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

fn environment_name(name: &str) -> bool {
    [
        b"KEY".as_slice(),
        b"TOKEN",
        b"SECRET",
        b"PASSWORD",
        b"PASSWD",
    ]
    .iter()
    .any(|word| {
        name.as_bytes()
            .windows(word.len())
            .any(|window| window.eq_ignore_ascii_case(word))
    })
}

fn unsafe_output(bytes: &[u8], literals: &[Literal]) -> bool {
    literals.iter().any(|literal| {
        bytes
            .windows(literal.value.bytes().len())
            .any(|window| window == literal.value.bytes())
    }) || (0..bytes.len()).any(|start| {
        BUILTINS
            .iter()
            .any(|builtin| bytes.len() - start >= builtin.len() && builtin.matches(&bytes[start..]))
    })
}

// Compute fallback from public delimiter text; no secret byte is copied here.
fn public_fallback(end: &[u8], position: usize) -> usize {
    (1..position)
        .rev()
        .find(|&length| end[..length] == end[position - length..position])
        .unwrap_or(0)
}
