use crate::SessionDescriptor;
use anyhow::anyhow;
use rtp_rs::RtpReader;
use socket2::{Domain, Socket, Type};
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::{
    net::UdpSocket,
    select, spawn,
    sync::{
        broadcast,
        mpsc::{self},
    },
    time::Instant,
};

pub struct Stream {
    pub descriptor: SessionDescriptor,
    pub socket: Option<UdpSocket>,
}

impl Stream {
    pub async fn new(
        descriptor: SessionDescriptor,
        local_address: Ipv4Addr,
    ) -> anyhow::Result<Self> {
        let addr = SocketAddrV4::new(descriptor.multicast_address, descriptor.multicast_port);
        let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
        socket.join_multicast_v4(&descriptor.multicast_address, &local_address)?;
        socket.set_reuse_address(true)?;
        socket.bind(&addr.into())?;
        socket.set_nonblocking(true)?;
        let socket = UdpSocket::from_std(socket.into())?;

        Ok(Stream {
            descriptor,
            socket: Some(socket),
        })
    }

    pub async fn play(
        &mut self,
        tx: mpsc::UnboundedSender<Vec<u8>>,
        stop: broadcast::Sender<()>,
    ) -> anyhow::Result<()> {
        let mut buf = [0; 102400];

        let mut start = Instant::now();
        let mut counter = 0;

        let socket = self
            .socket
            .take()
            .ok_or(anyhow!("receiver already started"))?;

        let mut stop = stop.subscribe();

        spawn(async move {
            let mut previous_sequence_number = None;
            loop {
                select! {
                    _ = stop.recv() => { break; },
                    recv = receive_rtp_payload(&socket, &mut buf) => {
                        match recv {
                            Ok(Some((payload,sequence_number))) => {

                                if let Some(previous_sequence_number) = previous_sequence_number {
                                    let diff = sequence_number - previous_sequence_number;
                                    if diff < 1 && !(sequence_number == 0 && previous_sequence_number == 65535) {
                                        log::warn!("Inconsistent RTP sequence number '{sequence_number}', previous was {previous_sequence_number}")
                                    } else if diff > 1 {
                                        log::warn!("Detected packet loss, {} packet(s) were not received", diff-1);
                                    }
                                }
                                previous_sequence_number = Some(sequence_number);

                                if start.elapsed().as_secs_f32() >= 1.0 {
                                    log::debug!(
                                        "Receiving {} packets/s; payload size: {}",
                                        counter,
                                        payload.len()
                                    );
                                    counter = 0;
                                    start = Instant::now();
                                } else {
                                    counter += 1;
                                }
                                if let Err(e) = tx.send(payload) {
                                    log::error!("Error forwarding received data: {e}");
                                    log::warn!("Stopping receiver.");
                                    break;
                                }
                            }
                            Ok(None) => (),
                            Err(e) => {
                                log::error!("Error receiving data: {e}");
                                log::warn!("Stopping receiver.");
                                break;
                            }
                        }
                    }
                }
            }
            log::info!("Receiver closed.");
        });

        Ok(())
    }
}

async fn receive_rtp_payload(
    sock: &UdpSocket,
    buf: &mut [u8],
) -> anyhow::Result<Option<(Vec<u8>, i32)>> {
    let len = sock.recv(buf).await?;
    if len > 0 {
        let rtp = RtpReader::new(&buf[0..len]).map_err(|e| anyhow!("{e:?}"))?;
        let end = rtp.payload().len() - rtp.padding().unwrap_or(0) as usize;
        let data = (&rtp.payload()[0..end]).to_owned();
        let sequence_number: u16 = rtp.sequence_number().into();
        Ok(Some((data, sequence_number as i32)))
    } else {
        Ok(None)
    }
}
