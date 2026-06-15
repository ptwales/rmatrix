use std::fs::File;
use std::io::{BufRead, BufReader};
use std::mem::swap;

#[derive(Clone, Debug, Default)]
pub struct NetStats {
    pub tx_packets: u64,
    pub rx_packets: u64,
}

impl NetStats {
    // consider blowing up if we can't read or find the network device
    fn read(nic: &str) -> Self {
        // Do cat this file just to see what we're dealing with.
        // TODO: feature flag unix and check if that's correct for MacOS
        let Ok(file) = File::open("/proc/net/dev") else {
            return Self::default();
        };
        // Manual text processing ALERT
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(pos) = line.find(nic) {
                let after_nic = &line[pos + nic.len()..];
                let after_colon = after_nic.strip_prefix(':').unwrap_or(after_nic);
                let mut stats = after_colon.split_whitespace(); // doing so much work (split \s+)
                let rx_packets = read_stat(stats.nth(1)); // 1th
                let tx_packets = read_stat(stats.nth(7)); // 9th
                return Self {
                    tx_packets,
                    rx_packets,
                };
            }
        }
        Self::default()
    }
}

fn read_stat(s: Option<&str>) -> u64 {
    s.and_then(|i| i.parse().ok()).unwrap_or(0)
}

pub struct NetState {
    net_interface: String,
    stats: NetStats,
}

impl NetState {
    pub fn new(net_interface: String) -> Self {
        let stats = NetStats::read(&net_interface);
        Self {
            net_interface,
            stats,
        }
    }

    /// Update with the latest stats and return the difference.
    pub fn poll(&mut self) -> NetStats {
        // self.stats = previous
        let mut stats = NetStats::read(&self.net_interface); // stats := current
        swap(&mut self.stats, &mut stats); // self.stats = current, stats := previous
        NetStats {
            tx_packets: self.stats.tx_packets - stats.tx_packets, // current - previous
            rx_packets: self.stats.rx_packets - stats.rx_packets, // current - previous
        }
    }
}
