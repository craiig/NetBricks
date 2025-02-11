#![feature(box_syntax)]
extern crate e2d2;
extern crate fnv;
extern crate getopts;
extern crate rand;
extern crate time;
use self::nat::*;
use self::acl::*;
use e2d2::allocators::CacheAligned;
use e2d2::config::{basic_opts, read_matches};
use e2d2::interface::*;
use e2d2::operators::*;
use e2d2::scheduler::*;
use e2d2::utils::Ipv4Prefix;
use std::env;
use std::fmt::Display;
use std::net::Ipv4Addr;
use std::process;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
mod nat;
mod acl;

const CONVERSION_FACTOR: f64 = 1000000000.;

fn test<S: Scheduler + Sized>(ports: Vec<CacheAligned<PortQueue>>, sched: &mut S) {
// fn test<T, S: Scheduler + Sized>(ports: Vec<T>, sched: &mut S) {
    for port in &ports {
        println!(
            "Receiving port {} rxq {} txq {}",
            port.port.mac_address(),
            port.rxq(),
            port.txq()
        );
    }
    let acls = vec![
        Acl {
            src_ip: Some(Ipv4Prefix::new(0x0a000001, 32)), // 10.0.0.1/32
            dst_ip: None,
            src_port: None,
            dst_port: None,
            established: None,
            drop: true, 
        },
        Acl {
            src_ip: Some(Ipv4Prefix::new(0, 0)),
            dst_ip: None,
            src_port: None,
            dst_port: None,
            established: None,
            drop: false, 
        },
    ];
    let mut pipelines: Vec<_> = ports
        .iter()
        .map(|port| {
	    let aclret = acl_match(ReceiveBatch::new(port.clone()), acls.clone());
            nat::nat(
                aclret,
                sched,
                &Ipv4Addr::new(10, 0, 0, 2), // nat address should not conflict with drop packets below
            ).send(port.clone())
        })
        .collect();

    println!("Running {} pipelines", pipelines.len());
    for pipeline in pipelines {
        sched.add_task(pipeline).unwrap();
    }
}

fn main() {
    let opts = basic_opts();

    let args: Vec<String> = env::args().collect();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => panic!(f.to_string()),
    };
    let configuration = read_matches(&matches, &opts);

    match initialize_system(&configuration) {
        Ok(mut context) => {
            context.start_schedulers();
            context.add_pipeline_to_run(Arc::new(move |p, s: &mut StandaloneScheduler| test(p, s)));
            context.execute();

            let mut pkts_so_far = (0, 0);
            let mut start = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
            let sleep_time = Duration::from_millis(500);
            loop {
                thread::sleep(sleep_time); // Sleep for a bit
                let now = time::precise_time_ns() as f64 / CONVERSION_FACTOR;
                if now - start > 1.0 {
                    let mut rx = 0;
                    let mut tx = 0;
                    for port in context.ports.values() {
                        for q in 0..port.rxqs() {
                            let (rp, tp) = port.stats(q);
                            rx += rp;
                            tx += tp;
                        }
                    }
                    let pkts = (rx, tx);
                    println!(
                        "{:.2} OVERALL RX {:.2} TX {:.2}",
                        now - start,
                        (pkts.0 - pkts_so_far.0) as f64 / (now - start),
                        (pkts.1 - pkts_so_far.1) as f64 / (now - start)
                    );
                    start = now;
                    pkts_so_far = pkts;
                }
            }
        }
        Err(ref e) => {
            println!("Error: {}", e);
            if let Some(backtrace) = e.backtrace() {
                println!("Backtrace: {:?}", backtrace);
            }
            process::exit(1);
        }
    }
}
