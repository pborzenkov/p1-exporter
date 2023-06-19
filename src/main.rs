use clap::Parser;
use dsmr5::state::Slave;
use prometheus_client::{
    encoding::text::encode,
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};
use std::{
    io::{self, Read},
    net::{SocketAddr, TcpStream},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};
use tiny_http::{Response, Server};

#[derive(Parser)]
#[clap(author, version, about)]
struct Args {
    #[clap(
        short,
        long,
        help = "Address to listen on",
        default_value = "127.0.0.1:4545"
    )]
    address: SocketAddr,
    #[clap(short, long, help = "P1 reader address")]
    p1_address: SocketAddr,
}

#[derive(Default)]
struct P1Metrics {
    power_consumed: Gauge<f64, AtomicU64>,
    power_produced: Gauge<f64, AtomicU64>,

    power_consumed_total: Family<[(&'static str, &'static str); 1], Counter<f64, AtomicU64>>,
    power_produced_total: Family<[(&'static str, &'static str); 1], Counter<f64, AtomicU64>>,

    active_tariff: Family<[(&'static str, &'static str); 1], Gauge>,

    gas_consumed_total: Counter<f64, AtomicU64>,
}

fn main() {
    let args = Args::parse();

    let mut registry = <Registry>::default();
    let metrics = <P1Metrics>::default();

    registry.register(
        "p1_power_consumed_kw",
        "Power consumed",
        metrics.power_consumed.clone(),
    );
    registry.register(
        "p1_power_produced_kw",
        "Power produced",
        metrics.power_produced.clone(),
    );
    registry.register(
        "p1_power_consumed_kwh",
        "Total consumed power",
        metrics.power_consumed_total.clone(),
    );
    registry.register(
        "p1_power_produced_kwh",
        "Total produced power",
        metrics.power_produced_total.clone(),
    );
    registry.register(
        "p1_active_tariff",
        "Currently active tariff",
        metrics.active_tariff.clone(),
    );
    registry.register(
        "p1_gas_consumed_cubic_meters",
        "Total consumed natural gas",
        metrics.gas_consumed_total.clone(),
    );

    start_metrics_collector(args.p1_address, Arc::new(metrics));
    if let Err(err) = run_metrics_server(args.address, registry) {
        eprintln!("terminating: {err}")
    }
}

fn start_metrics_collector(addr: SocketAddr, metrics: Arc<P1Metrics>) {
    thread::spawn(move || loop {
        match TcpStream::connect(addr) {
            Ok(sock) => {
                if let Err(err) = collect_metrics(sock, metrics.clone()) {
                    eprintln!("Failed to collect metrics: {err}");
                }
            }
            Err(err) => {
                eprintln!("Failed to connect to P1 reader: {err}")
            }
        };
        thread::sleep(Duration::from_secs(5));
    });
}

fn collect_metrics(sock: TcpStream, metrics: Arc<P1Metrics>) -> Result<(), io::Error> {
    sock.set_read_timeout(Some(Duration::from_secs(2)))?;
    let reader = dsmr5::Reader::new(sock.bytes().map_while(|b| b.ok()));

    for readout in reader {
        let telegram = readout
            .to_telegram()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?;
        let state = dsmr5::Result::<dsmr5::state::State>::from(&telegram)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("{:?}", e)))?;

        if let Some(pd) = state.power_delivered {
            metrics.power_consumed.set(pd);
        }
        if let Some(pd) = state.power_received {
            metrics.power_produced.set(pd);
        }

        if let Some(pd) = state.meterreadings[0].to {
            metrics
                .power_consumed_total
                .get_or_create(&[("tariff", "low")])
                .inner()
                .store(pd.to_bits(), Ordering::SeqCst);
        }
        if let Some(pd) = state.meterreadings[1].to {
            metrics
                .power_consumed_total
                .get_or_create(&[("tariff", "high")])
                .inner()
                .store(pd.to_bits(), Ordering::SeqCst);
        }

        if let Some(pd) = state.meterreadings[0].by {
            metrics
                .power_produced_total
                .get_or_create(&[("tariff", "low")])
                .inner()
                .store(pd.to_bits(), Ordering::SeqCst);
        }
        if let Some(pd) = state.meterreadings[1].by {
            metrics
                .power_produced_total
                .get_or_create(&[("tariff", "high")])
                .inner()
                .store(pd.to_bits(), Ordering::SeqCst);
        }

        metrics.active_tariff.clear();
        match state.tariff_indicator {
            Some([0, 1]) => metrics
                .active_tariff
                .get_or_create(&[("tariff", "low")])
                .set(1),
            Some([0, 2]) => metrics
                .active_tariff
                .get_or_create(&[("tariff", "high")])
                .set(1),
            _ => 0,
        };

        for sl in state.slaves {
            if let Slave {
                device_type: Some(3),
                meter_reading: Some((_, gd)),
            } = sl
            {
                metrics
                    .gas_consumed_total
                    .inner()
                    .store(gd.to_bits(), Ordering::SeqCst);
            }
        }
    }

    Ok(())
}

fn run_metrics_server(addr: SocketAddr, registry: Registry) -> Result<(), io::Error> {
    let content_type = "Content-Type: application/openmetrics-text; version=1.0.0; charset=utf-8"
        .parse::<tiny_http::Header>()
        .unwrap();
    let server = Server::http(addr).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    for req in server.incoming_requests() {
        let mut body = String::new();
        let response = match encode(&mut body, &registry) {
            Ok(()) => Response::from_string(body).with_header(content_type.clone()),
            Err(err) => Response::from_string(format!("{}", err)).with_status_code(500),
        };
        if let Err(err) = req.respond(response) {
            eprintln!("failed to respond: {err}");
        }
    }

    Ok(())
}
