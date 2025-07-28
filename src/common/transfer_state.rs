pub enum TransferType {
    Sending,
    Receiving
}

pub struct TransferState {
    pub ttype: TransferType,
    pub original_filepath: String,
    pub dest_filepath: String,
    pub percentage: f32,
    pub peer: std::net::SocketAddr
}

impl Default for TransferState {
    fn default() -> Self {
        TransferState {
            ttype: TransferType::Sending,
            original_filepath: String::new(),
            dest_filepath: String::new(),
            percentage: 0.0,
            peer: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            )
        }
    }
}