use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::collections::HashSet;
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CodeError {
    #[error("No channels available")]
    ChannelNotAvailable,
}

// TODO: bigger wordlist.
const WORDLIST: &[&str] = &[
    "apple",
    "banana",
    "cherry",
    "date",
    "elderberry",
    "fig",
    "grape",
    "honeydew",
];

impl FromStr for Code {
    type Err = CodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('-');
        let channel_number = parts
            .next()
            .ok_or(CodeError::ChannelNotAvailable)?
            .parse()
            .map_err(|_| CodeError::ChannelNotAvailable)?;
        let word1 = parts
            .next()
            .ok_or(CodeError::ChannelNotAvailable)?
            .to_string();
        let word2 = parts
            .next()
            .ok_or(CodeError::ChannelNotAvailable)?
            .to_string();
        Ok(Self {
            channel_number,
            word1,
            word2,
        })
    }
}

impl ToString for Code {
    fn to_string(&self) -> String {
        format!("{}-{}-{}", self.channel_number, self.word1, self.word2)
    }
}

#[derive(Debug, Clone)]
pub struct Code {
    pub channel_number: u8,
    pub word1: String,
    pub word2: String,
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        // TODO: could we implement a global sleep here to prevent a timing attack :think:
        self.channel_number == other.channel_number
            && self.word1 == other.word1
            && self.word2 == other.word2
    }
}

impl Code {
    fn new(channel_number: u8, word1: &str, word2: &str) -> Self {
        Self {
            channel_number,
            word1: word1.to_string(),
            word2: word2.to_string(),
        }
    }
}

pub struct CodeGenerator {
    rng: ChaCha8Rng,
    available_channels: HashSet<u8>,
    pending_channels: HashSet<u8>,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            rng: ChaCha8Rng::from_entropy(),
            available_channels: (0..100).collect(),
            pending_channels: HashSet::new(),
        }
    }

    pub fn generate(&mut self) -> Result<Code, CodeError> {
        // TODO: get an actual random element from the set.
        let channel_number = self
            .available_channels
            .iter()
            .next()
            .copied()
            .ok_or(CodeError::ChannelNotAvailable)?;
        self.pending_channels.insert(channel_number);
        self.available_channels.remove(&channel_number);
        let word1 = WORDLIST[self.rng.gen_range(0..WORDLIST.len())];
        let word2 = WORDLIST[self.rng.gen_range(0..WORDLIST.len())];

        Ok(Code::new(channel_number, word1, word2))
    }

    pub fn release(&mut self, channel_number: u8) {
        self.available_channels.insert(channel_number);
        self.pending_channels.remove(&channel_number);
    }
}
