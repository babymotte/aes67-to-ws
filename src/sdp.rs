use crate::{BitDepth, SessionDescriptor};
use anyhow::anyhow;
use regex::Regex;
use std::{net::Ipv4Addr, str::FromStr};

const RTPMAP_REGEX: &str = r"rtpmap:([0-9]+) (.+)\/([0-9]+)\/([0-9]+)";
const RTPMAP_PAYLOAD_ID_GROUPT: usize = 1;
const RTPMAP_BITDEPTH_GROUPT: usize = 2;
const RTPMAP_SAMPLERATE_GROUPT: usize = 3;
const RTPMAP_CHANNELS_GROUPT: usize = 4;

const MEDIA_AND_TRANSPORT_REGEX: &str = r"(.+) ([0-9]+) (.+) ([0-9]+)";
const MEDIA_AND_TRANSPORT_MEDIA_GROUP: usize = 1;
const MEDIA_AND_TRANSPORT_PORT_GROUP: usize = 2;
const MEDIA_AND_TRANSPORT_PROTOCOL_GROUP: usize = 3;
const MEDIA_AND_TRANSPORT_PAYLOAD_ID_GROUP: usize = 4;

const CONNECTION_INFO_REGEX: &str = r"(.+) (IP[4,6]) ([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+)\/([0-9]+)";
const CONNECTION_INFO_MULTICAST_GROUP: usize = 3;

const PTIME_REGEX: &str = r"ptime:(.+)";
const PTIME_GROUP: usize = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct RtpMap {
    payload_id: u16,
    bit_depth: BitDepth,
    sample_rate: u32,
    channels: u16,
}

impl FromStr for RtpMap {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(RTPMAP_REGEX).expect("cannot fail");
        if let Some(caps) = re.captures(s) {
            Ok(RtpMap {
                payload_id: caps
                    .get(RTPMAP_PAYLOAD_ID_GROUPT)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
                bit_depth: caps
                    .get(RTPMAP_BITDEPTH_GROUPT)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
                sample_rate: caps
                    .get(RTPMAP_SAMPLERATE_GROUPT)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
                channels: caps
                    .get(RTPMAP_CHANNELS_GROUPT)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
            })
        } else {
            Err(anyhow!("malformed rtpmap: {s}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaAndTransport {
    media: Media,
    port: u16,
    protocol: String,
    payload_id: u16,
}

impl FromStr for MediaAndTransport {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(MEDIA_AND_TRANSPORT_REGEX).expect("cannot fail");
        if let Some(caps) = re.captures(s) {
            Ok(MediaAndTransport {
                media: caps
                    .get(MEDIA_AND_TRANSPORT_MEDIA_GROUP)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
                port: caps
                    .get(MEDIA_AND_TRANSPORT_PORT_GROUP)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
                protocol: caps
                    .get(MEDIA_AND_TRANSPORT_PROTOCOL_GROUP)
                    .expect("must exist in matches")
                    .as_str()
                    .to_owned(),
                payload_id: caps
                    .get(MEDIA_AND_TRANSPORT_PAYLOAD_ID_GROUP)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
            })
        } else {
            Err(anyhow!("malformed media/transport: {s}"))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Media {
    Audio,
    Video,
}

impl FromStr for Media {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "audio" => Ok(Media::Audio),
            "video" => Ok(Media::Video),
            _ => Err(anyhow!("unsupported media type: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectionInfo {
    multicast_address: Ipv4Addr,
}

impl FromStr for ConnectionInfo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let re = Regex::new(CONNECTION_INFO_REGEX).expect("cannot fail");
        if let Some(caps) = re.captures(s) {
            Ok(ConnectionInfo {
                multicast_address: caps
                    .get(CONNECTION_INFO_MULTICAST_GROUP)
                    .expect("must exist in matches")
                    .as_str()
                    .parse()?,
            })
        } else {
            Err(anyhow!("malformed connection info: {s}"))
        }
    }
}

fn parse_packet_time(attribute: &str) -> anyhow::Result<f32> {
    let re = Regex::new(PTIME_REGEX).expect("cannot fail");
    if let Some(caps) = re.captures(attribute) {
        Ok(caps
            .get(PTIME_GROUP)
            .expect("must exist in matches")
            .as_str()
            .parse()?)
    } else {
        Err(anyhow!("malformed ptime: {attribute}"))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SdpValue {
    OriginatorAndSessionIdentifier(String),          // o
    SessionName(String),                             // s
    ActiveTime((usize, usize)),                      // t
    MediaNameAndTransportAddress(MediaAndTransport), // m
    SessionInfo(String),                             // i
    SessionDescription(String),                      // u
    ConnectionInformation(ConnectionInfo),           // c
    Attribute(String),                               // a
}

fn parse_line(line: &str) -> anyhow::Result<Option<(&str, SdpValue)>> {
    let trim = line.trim();

    if trim.starts_with("#") || trim.is_empty() {
        return Ok(None);
    }

    let mut kv = trim.split("=");
    if let (Some(key), Some(value)) = (kv.next(), kv.next()) {
        if let Some(value) = parse_value(key, value)? {
            Ok(Some((key, value)))
        } else {
            Ok(None)
        }
    } else {
        Err(anyhow!("line is not a key/value pair: {line}"))
    }
}

fn parse_value(key: &str, value: &str) -> anyhow::Result<Option<SdpValue>> {
    match key {
        "o" => Ok(Some(SdpValue::OriginatorAndSessionIdentifier(
            value.to_owned(),
        ))),
        "s" => Ok(Some(SdpValue::SessionName(value.to_owned()))),
        "m" => Ok(Some(SdpValue::MediaNameAndTransportAddress(value.parse()?))),
        "i" => Ok(Some(SdpValue::SessionInfo(value.to_owned()))),
        "u" => Ok(Some(SdpValue::SessionDescription(value.to_owned()))),
        "c" => Ok(Some(SdpValue::ConnectionInformation(value.parse()?))),
        "a" => Ok(Some(SdpValue::Attribute(value.to_owned()))),
        _ => Ok(None),
    }
}

impl FromStr for SessionDescriptor {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let lines = s.split("\n");

        let mut bit_depth = None;
        let mut channels = None;
        let mut multicast_address = None;
        let mut multicast_port = None;
        let mut packet_time = None;
        let mut sample_rate = None;

        for line in lines {
            if let Some((_, value)) = parse_line(line)? {
                match value {
                    SdpValue::OriginatorAndSessionIdentifier(_) => {}
                    SdpValue::SessionName(_) => {}
                    SdpValue::ActiveTime(_) => {}
                    SdpValue::MediaNameAndTransportAddress(m) => {
                        multicast_port = Some(m.port);
                    }
                    SdpValue::SessionInfo(_) => {}
                    SdpValue::SessionDescription(_) => {}
                    SdpValue::ConnectionInformation(c) => {
                        multicast_address = Some(c.multicast_address)
                    }
                    SdpValue::Attribute(a) => {
                        if let Ok(rtpmap) = a.parse::<RtpMap>() {
                            sample_rate = Some(rtpmap.sample_rate);
                            channels = Some(rtpmap.channels);
                            bit_depth = Some(rtpmap.bit_depth);
                        }
                        if let Ok(ptime) = parse_packet_time(&a) {
                            packet_time = Some(ptime);
                        }
                    }
                }
            }
        }

        if let (
            Some(bit_depth),
            Some(channels),
            Some(multicast_address),
            Some(multicast_port),
            Some(packet_time),
            Some(sample_rate),
        ) = (
            bit_depth,
            channels,
            multicast_address,
            multicast_port,
            packet_time,
            sample_rate,
        ) {
            Ok(SessionDescriptor {
                bit_depth,
                channels,
                multicast_address,
                multicast_port,
                packet_time,
                sample_rate,
            })
        } else {
            Err(anyhow!("malformed SDP: {s}"))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_comment() {
        let line = "# hello world";
        let parsed = parse_line(line).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_empty_line() {
        let line = " ";
        let parsed = parse_line(line).unwrap();
        assert!(parsed.is_none());
    }

    #[test]
    fn parse_name_and_transport() {
        let line = "m=audio 5004 RTP/AVP 98";
        let (key, value) = parse_line(line).unwrap().unwrap();
        assert_eq!(key, "m");
        assert_eq!(
            value,
            SdpValue::MediaNameAndTransportAddress(MediaAndTransport {
                media: Media::Audio,
                port: 5004,
                protocol: "RTP/AVP".to_owned(),
                payload_id: 98
            })
        );
    }

    #[test]
    fn parse_attribute() {
        let line = "a=rtpmap:98 L16/48000/8";
        let (key, value) = parse_line(line).unwrap().unwrap();
        assert_eq!(key, "a");
        assert_eq!(
            value,
            SdpValue::Attribute("rtpmap:98 L16/48000/8".to_owned())
        );
    }

    #[test]
    fn parse_rtpmap() {
        let line = "rtpmap:98 L16/48000/8";
        let rtp_map: RtpMap = line.parse().unwrap();
        assert_eq!(
            rtp_map,
            RtpMap {
                bit_depth: BitDepth::L16,
                channels: 8,
                payload_id: 98,
                sample_rate: 48000
            }
        );
    }
}
