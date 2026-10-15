//! Speed Test through Ookla's `speedtest.exe`, exercised through the toolbox core's public
//! interface (seam S1) over a scripted network (seam S2) that prints what the program prints.

mod common;

use std::time::Duration;

use common::{FakeNetwork, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::{OoklaFailure, PublicIp};
use toolbox_core::speed_test::{
    self, Provider, Sample, SpeedPhase, SpeedTest, SpeedTestEnding, SpeedTestEvent,
    SpeedTestRequest, SpeedTestResult,
};
use toolbox_core::target::AddressFamily;

const SERVER: &str = r#""server":{"id":1,"host":"speedtest.example.net","name":"Example Telecom","location":"Cebu","country":"Philippines","ip":"198.51.100.4"}"#;

fn start() -> String {
    format!(
        r#"{{"type":"testStart","isp":"Example ISP","interface":{{"externalIp":"203.0.113.7"}},{SERVER}}}"#
    )
}

const PING: &str = r#"{"type":"ping","ping":{"jitter":2.1,"latency":14.5,"progress":1}}"#;
const DOWNLOADING: &str = r#"{"type":"download","download":{"bandwidth":25000000,"bytes":50000000,"elapsed":2000,"progress":0.2}}"#;
const UPLOADING: &str = r#"{"type":"upload","upload":{"bandwidth":5000000,"bytes":10000000,"elapsed":2000,"progress":0.2}}"#;

fn finished() -> String {
    format!(
        r#"{{"type":"result","ping":{{"jitter":2.1,"latency":14.5}},"download":{{"bandwidth":28000000,"bytes":280000000,"elapsed":10000,"latency":{{"iqm":40.2,"low":12.0,"high":90.0,"jitter":5.0}}}},"upload":{{"bandwidth":6000000,"bytes":60000000,"elapsed":10000,"latency":{{"iqm":120.5}}}},"isp":"Example ISP","interface":{{"externalIp":"203.0.113.7"}},{SERVER},"result":{{"url":"https://www.speedtest.net/result/c/example"}}}}"#
    )
}

fn ookla_test() -> SpeedTest {
    SpeedTest {
        provider: Provider::Ookla,
        ..SpeedTest::default()
    }
}

async fn run(network: &FakeNetwork) -> SpeedTestResult {
    speed_test::run(network, &ookla_test(), &CancellationToken::new(), |_| {}).await
}

fn complete_run() -> FakeNetwork {
    let start = start();
    let finished = finished();
    FakeNetwork::new().with_ookla(&[&start, PING, DOWNLOADING, UPLOADING, &finished])
}

#[tokio::test(start_paused = true)]
async fn the_finished_line_becomes_the_result() {
    let result = run(&complete_run()).await;

    assert_eq!(result.ending, SpeedTestEnding::Completed);
    // Ookla reports bandwidth in bytes per second.
    assert_eq!(result.download.unwrap().bits_per_second, 224_000_000);
    assert_eq!(result.upload.unwrap().bits_per_second, 48_000_000);
    let latency = result.latency.unwrap();
    assert_eq!(latency.median, Duration::from_micros(14_500));
    assert_eq!(latency.jitter, Duration::from_micros(2_100));
    assert_eq!(result.download_loaded, Some(Duration::from_micros(40_200)));
    assert_eq!(result.upload_loaded, Some(Duration::from_micros(120_500)));
    assert_eq!(result.data_used, 340_000_000);
}

#[tokio::test(start_paused = true)]
async fn the_server_and_public_address_come_from_ookla() {
    let header = run(&complete_run()).await.header;

    let server = header.server.unwrap();
    assert_eq!(server.provider, "Ookla");
    assert_eq!(server.location, "Example Telecom, Cebu");
    assert_eq!(server.address, Some(common::ip("198.51.100.4")));
    assert_eq!(
        header.public,
        Some(Ok(PublicIp {
            address: common::ip("203.0.113.7"),
            isp: Some("Example ISP".to_string()),
            city: None,
        }))
    );
}

#[tokio::test(start_paused = true)]
async fn progress_lines_become_phases_and_live_speed_samples() {
    let mut events = Vec::new();

    speed_test::run(
        &complete_run(),
        &ookla_test(),
        &CancellationToken::new(),
        |event| events.push(event),
    )
    .await;

    assert_eq!(events[0], SpeedTestEvent::Phase(SpeedPhase::Latency));
    assert!(events.contains(&SpeedTestEvent::Phase(SpeedPhase::Download)));
    assert!(events.contains(&SpeedTestEvent::Sample(Sample {
        phase: SpeedPhase::Download,
        at: Duration::from_secs(2),
        bits_per_second: 200_000_000,
    })));
    assert!(events.contains(&SpeedTestEvent::Phase(SpeedPhase::Upload)));
}

#[tokio::test(start_paused = true)]
async fn a_completed_ookla_run_is_rated_like_any_other() {
    let result = run(&complete_run()).await;

    assert!(result.ratings().iter().all(|rating| rating.stars.is_some()));
    let text = result.to_text();
    assert!(
        text.contains("Server:    Ookla Example Telecom, Cebu (198.51.100.4)"),
        "{text}"
    );
    assert!(text.contains("Download:      224.0 Mbps"), "{text}");
    assert!(
        text.contains("under load:  40.2 ms (+25.7 ms, Low)"),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn a_missing_program_says_how_to_get_it() {
    let result = run(&FakeNetwork::new()).await;

    let SpeedTestEnding::Unavailable { reason } = result.ending else {
        panic!("{:?}", result.ending);
    };
    assert!(reason.contains("speedtest.exe"), "{reason}");
    assert!(reason.contains("install"), "{reason}");
    assert_eq!(result.download, None);
}

#[tokio::test(start_paused = true)]
async fn a_program_that_fails_reports_what_it_said_and_keeps_what_it_measured() {
    let start = start();
    let network = FakeNetwork::new().with_ookla_failing(
        &[&start, PING, DOWNLOADING],
        OoklaFailure::Failed {
            message: "You must accept the license agreement".to_string(),
        },
    );

    let result = run(&network).await;

    assert_eq!(
        result.ending,
        SpeedTestEnding::Unavailable {
            reason: "You must accept the license agreement".to_string()
        }
    );
    assert_eq!(
        result.latency.unwrap().median,
        Duration::from_micros(14_500)
    );
    assert_eq!(result.download, None);
}

#[tokio::test(start_paused = true)]
async fn stopping_ends_the_program_and_keeps_what_was_measured() {
    let start = start();
    let network = FakeNetwork::new().with_ookla_hanging(&[&start, PING, DOWNLOADING]);
    let stop = CancellationToken::new();
    let stopper = stop.clone();
    tokio::spawn(async move {
        tokio::time::sleep(ms(500)).await;
        stopper.cancel();
    });

    let result = speed_test::run(&network, &ookla_test(), &stop, |_| {}).await;

    assert_eq!(result.ending, SpeedTestEnding::Stopped);
    assert!(result.latency.is_some());
    assert_eq!(result.download, None);
}

#[tokio::test(start_paused = true)]
async fn ookla_is_only_used_when_asked_for() {
    // A network with Ookla installed still gets the default provider's requests.
    let request = SpeedTestRequest {
        family: AddressFamily::Auto,
        provider: Provider::default(),
        plan_download: String::new(),
        plan_upload: String::new(),
    };

    assert_eq!(request.validate().unwrap().provider, Provider::Cloudflare);
}

#[tokio::test(start_paused = true)]
async fn the_technician_can_ask_whether_ookla_is_installed() {
    assert!(speed_test::ookla_installed(&complete_run()).await);
    assert!(!speed_test::ookla_installed(&FakeNetwork::new()).await);
}
