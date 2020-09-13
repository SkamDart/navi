use futures::stream::StreamExt;

use rtnetlink::{
    new_connection,
    packet::{rtnl::link::nlas::Nla, LinkMessage, NetlinkPayload, RtnlMessage, RtnlMessage::*},
    sys::{constants::*, SocketAddr},
};

use zoomies::{Client, ConfigBuilder, Event};

#[tokio::main(max_threads = 1, core_threads = 1)]
async fn main() -> Result<(), String> {
    // conn - `Connection` that has a netlink socket which is a `Future` that polls the socket
    // and thus must have an event loop
    //
    // handle - `Handle` to the `Connection`. Used to send/recv netlink messages.
    //
    // messages - A channel receiver.
    let (mut conn, mut _handle, mut messages) = new_connection().map_err(|e| format!("{}", e))?;

    // Create a datadog client.
    let dd = Client::with_config(ConfigBuilder::new().finish())
        .await
        .map_err(|e| format!("{}", e))?;

    // These flags specify what kinds of broadcast messages we want to listen for.
    let groups = RTNLGRP_LINK
        | RTNLGRP_IPV4_IFADDR
        | RTNLGRP_IPV6_IFADDR
        | RTNLGRP_IPV4_ROUTE
        | RTNLGRP_IPV6_ROUTE
        | RTNLGRP_MPLS_ROUTE
        | RTNLGRP_IPV4_MROUTE
        | RTNLGRP_IPV6_MROUTE
        | RTNLGRP_NEIGH
        | RTNLGRP_IPV4_NETCONF
        | RTNLGRP_IPV6_NETCONF
        | RTNLGRP_IPV4_RULE
        | RTNLGRP_IPV6_RULE
        | RTNLGRP_NSID
        | RTNLGRP_MPLS_NETCONF;

    // Create new socket that listens for the messages described above.
    let addr = SocketAddr::new(0, groups);
    conn.socket_mut().bind(&addr).expect("Failed to bind");

    // Spawn `Connection` to start polling netlink socket.
    tokio::spawn(conn);

    // Start receiving events through `messages` channel.
    while let Some((message, _)) = messages.next().await {
        match message.payload {
            NetlinkPayload::Done => {
                println!("Done");
            }
            NetlinkPayload::Error(em) => {
                eprintln!("Error: {:?}", em);
            }
            NetlinkPayload::Ack(_am) => {}
            NetlinkPayload::Noop => {}
            NetlinkPayload::Overrun(_bytes) => {}
            NetlinkPayload::InnerMessage(m) => {
                handle_message(&dd, m).await;
            }
        }
    }
    Ok(())
}

fn find_ifname(lm: LinkMessage) -> Option<String> {
    for nla in lm.nlas.into_iter() {
        match nla {
            Nla::IfName(name) => return Some(name),
            _ => continue,
        }
    }
    None
}

async fn on_link_deleted(dd: &Client, lm: LinkMessage) {
    println!("Interface Deleted");
    if let Some(name) = find_ifname(lm) {
        println!("{:?} was deleted", name);
        let event = Event::new().title("Interface Deleted").text(&name).build().expect("nice");
        dd.send(&event).await.expect("failed");
    }
}

async fn on_link_created(dd: &Client, lm: LinkMessage) {
    if let Some(name) = find_ifname(lm) {
        println!("Interface {} is up", name);
        let event = Event::new().title("Interface Created").text(&name).build().expect("nice");
        dd.send(&event).await.expect("failed");
    }
}

async fn on_link_set(dd: &Client, lm: LinkMessage) {
    if let Some(name) = find_ifname(lm) {
        println!("Interface {:?} was set.", name);
        let event = Event::new().title("Interface Set").text(&name).build().expect("nice");
        dd.send(&event).await.expect("failed");
    }
}

async fn handle_message(dd: &Client, msg: RtnlMessage) {
    match msg {
        NewLink(lm) => on_link_created(dd, lm).await,
        DelLink(lm) => on_link_deleted(dd, lm).await,
        SetLink(lm) => on_link_set(dd, lm).await,
        GetLink(_lm) => {}
        NewAddress(_am) => {}
        DelAddress(_am) => {}
        GetAddress(_am) => {}
        NewNeighbour(_nm) => {}
        GetNeighbour(_nm) => {}
        DelNeighbour(_nm) => {}
        NewRule(_rm) => {}
        DelRule(_rm) => {}
        GetRule(_rm) => {}
        NewRoute(_rm) => {}
        DelRoute(_rm) => {}
        GetRoute(_rm) => {}
        _ => {
            // NewNeighbourTable(NeighbourTableMessage),
            // GetNeighbourTable(NeighbourTableMessage),
            // SetNeighbourTable(NeighbourTableMessage),
            // NewQueueDiscipline(TcMessage),
            // DelQueueDiscipline(TcMessage),
            // GetQueueDiscipline(TcMessage),
            // NewTrafficClass(TcMessage),
            // DelTrafficClass(TcMessage),
            // GetTrafficClass(TcMessage),
            // NewTrafficFilter(TcMessage),
            // DelTrafficFilter(TcMessage),
            // GetTrafficFilter(TcMessage),
            // NewTrafficChain(TcMessage),
            // DelTrafficChain(TcMessage),
            // GetTrafficChain(TcMessage),
            // NewNsId(NsidMessage),
            // DelNsId(NsidMessage),
            // GetNsId(NsidMessage),
            println!("Unhandled Message: {:?}", msg);
        }
    }
}
