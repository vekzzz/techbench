//! Speed Test, exercised through the toolbox core's public interface (seam S1) over a scripted network (seam S2).

mod common;

use std::time::Duration;

use common::{FakeNetwork, ms};
use tokio_util::sync::CancellationToken;
use toolbox_core::network::ConnectionCost;
use toolbox_core::network::{
    Adapter, AdapterAddress, AdapterKind, PublicIp, SpeedFailure, SpeedProbe, SpeedTestServer,
};
use toolbox_core::run::Progress;
use toolbox_core::speed_test::{
    self, Sample, SpeedPhase, SpeedTest, SpeedTestEnding, SpeedTestEvent, SpeedTestResult,
};
use toolbox_core::speed_test::{
    Bufferbloat, Latency, Plan, Provider, SpeedDirection, SpeedTestRequest, Stars, Throughput,
    UseCase,
};
use toolbox_core::target::AddressFamily;

fn probe(round_trip_ms: u64, server_ms: u64) -> SpeedProbe {
    SpeedProbe {
        round_trip: ms(round_trip_ms),
        server_time: ms(server_ms),
        server: SpeedTestServer {
            provider: "Cloudflare".to_string(),
            location: "HKG".to_string(),
            address: Some(common::ip("104.16.124.96")),
        },
    }
}

async fn run(network: &FakeNetwork) -> SpeedTestResult {
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    speed_test::run(network, &test, &CancellationToken::new(), |_| {}).await
}

#[tokio::test(start_paused = true)]
async fn idle_latency_is_the_median_of_twenty_probes_without_the_servers_own_time() {
    // Round trips 10..=29 ms, each carrying 2 ms of server time: latencies 8..=27 ms, whose
    // median is the mean of the 10th and 11th, (17 + 18) / 2 ms.
    let network = FakeNetwork::new()
        .with_probes((10..30).map(|round_trip| probe(round_trip, 2)))
        .with_download_rate(50)
        .with_upload_rate(10);

    let result = run(&network).await;

    let latency = result.latency.expect("latency was measured");
    assert_eq!(latency.median, Duration::from_micros(17_500));
}

#[tokio::test(start_paused = true)]
async fn jitter_is_the_mean_absolute_difference_between_consecutive_latencies() {
    // Latencies alternate 10 and 20 ms, so each of the 19 steps is 10 ms.
    let network = FakeNetwork::new()
        .with_probes((0..20).map(|step| probe(if step % 2 == 0 { 10 } else { 20 }, 0)))
        .with_download_rate(50)
        .with_upload_rate(10);

    let result = run(&network).await;

    assert_eq!(result.latency.expect("latency was measured").jitter, ms(10));
}

/// Twenty idle probes, so a test about the download need not care about them.
fn idle_probes() -> impl Iterator<Item = SpeedProbe> {
    (0..20).map(|_| probe(10, 0))
}

#[tokio::test(start_paused = true)]
async fn download_speed_is_the_total_across_connections_once_the_warm_up_is_over() {
    // Four connections at 50 Mbps each carry 200 Mbps.
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);

    let result = run(&network).await;

    let download = result.download.expect("download was measured");
    assert_eq!(download.bits_per_second, 200_000_000);
}

#[tokio::test(start_paused = true)]
async fn download_uses_four_connections_with_requests_well_under_a_hundred_megabytes() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);

    run(&network).await;

    assert_eq!(network.peak_downloads(), 4);
    assert!(
        network
            .download_requests()
            .iter()
            .all(|&bytes| bytes <= 25_000_000)
    );
}

#[tokio::test(start_paused = true)]
async fn a_server_that_cannot_be_reached_at_the_start_ends_the_run_as_no_internet() {
    let network = FakeNetwork::new().with_probe_failure(SpeedFailure::Unreachable);

    let result = run(&network).await;

    assert_eq!(result.ending, SpeedTestEnding::NoInternet);
    assert_eq!(result.latency, None);
}

#[tokio::test(start_paused = true)]
async fn a_rate_limit_ends_the_run_with_the_wait_the_server_asked_for() {
    let network = FakeNetwork::new().with_probe_failure(SpeedFailure::RateLimited {
        retry_after: Some(Duration::from_secs(30)),
    });

    let result = run(&network).await;

    assert_eq!(
        result.ending,
        SpeedTestEnding::RateLimited {
            retry_after: Some(Duration::from_secs(30))
        }
    );
}

#[tokio::test(start_paused = true)]
async fn a_download_that_breaks_keeps_the_latency_and_leaves_the_download_unmeasured() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_download_failure(SpeedFailure::Interrupted {
            reason: "connection reset".to_string(),
        });

    let result = run(&network).await;

    assert_eq!(
        result.ending,
        SpeedTestEnding::Interrupted {
            reason: "connection reset".to_string()
        }
    );
    assert!(result.latency.is_some());
    assert_eq!(result.download, None);
}

#[tokio::test(start_paused = true)]
async fn stopping_during_the_download_keeps_the_latency_and_leaves_the_download_unmeasured() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    let stop = CancellationToken::new();
    let stopper = stop.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        stopper.cancel();
    });

    let result = speed_test::run(&network, &test, &stop, |_| {}).await;

    assert_eq!(result.ending, SpeedTestEnding::Stopped);
    assert!(result.latency.is_some());
    assert_eq!(result.download, None);
}

#[tokio::test(start_paused = true)]
async fn data_used_counts_every_byte_moved_including_the_warm_up() {
    // Download: four connections at 50 Mbps for 10 s, 4 x 62.5 MB.
    // Upload: four connections at 10 Mbps for 10 s, 4 x 12.5 MB.
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);

    let result = run(&network).await;

    assert_eq!(result.data_used, 300_000_000);
}

#[tokio::test(start_paused = true)]
async fn a_forced_address_family_is_used_for_every_request() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = SpeedTest {
        family: AddressFamily::Ipv6,
        ..SpeedTest::default()
    };

    speed_test::run(&network, &test, &CancellationToken::new(), |_| {}).await;

    let families = network.speed_families();
    assert!(!families.is_empty());
    assert!(families.iter().all(|&family| family == AddressFamily::Ipv6));
}

fn wifi_carrying_the_default_route() -> Adapter {
    Adapter {
        name: "Wi-Fi".to_string(),
        description: "Wi-Fi controller".to_string(),
        kind: AdapterKind::Wireless,
        up: true,
        mac: None,
        addresses: vec![AdapterAddress {
            address: common::ip(common::PC_ADDRESS),
            prefix: 24,
        }],
        gateways: vec![common::ip("10.0.0.1")],
        dns_servers: vec![],
        dhcp: None,
        ipv4_metric: 25,
        ipv6_metric: 25,
    }
}

#[tokio::test(start_paused = true)]
async fn the_header_records_when_where_from_and_over_which_adapter_and_public_address() {
    let public = PublicIp {
        address: common::ip("203.0.113.7"),
        isp: Some("Example Telecom".to_string()),
        city: Some("Manila".to_string()),
    };
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_adapters(vec![wifi_carrying_the_default_route()])
        .with_public_ip(Ok(public.clone()));

    let header = run(&network).await.header;

    assert_eq!(header.started_at.to_rfc3339(), common::NOW);
    assert_eq!(header.pc_name, common::PC_NAME);
    assert_eq!(header.pc_address, Some(common::ip(common::PC_ADDRESS)));
    assert_eq!(header.adapter.as_deref(), Some("Wi-Fi"));
    assert_eq!(header.public, Some(Ok(public)));
}

#[tokio::test(start_paused = true)]
async fn a_failed_public_lookup_is_recorded_and_does_not_end_the_run() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_public_ip(Err("could not reach ipinfo.io".to_string()));

    let result = run(&network).await;

    assert_eq!(
        result.header.public,
        Some(Err("could not reach ipinfo.io".to_string()))
    );
    assert_eq!(result.ending, SpeedTestEnding::Completed);
    assert!(result.download.is_some());
}

#[tokio::test(start_paused = true)]
async fn the_result_copies_as_plain_text_with_a_header_for_the_ticket() {
    let network = FakeNetwork::new()
        .with_probes((10..30).map(|round_trip| probe(round_trip, 2)))
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_adapters(vec![wifi_carrying_the_default_route()])
        .with_public_ip(Ok(PublicIp {
            address: common::ip("203.0.113.7"),
            isp: Some("Example Telecom".to_string()),
            city: Some("Manila".to_string()),
        }));

    let text = run(&network).await.to_text();

    assert_eq!(
        text,
        "Speed Test
Server:    Cloudflare HKG (104.16.124.96)
Time:      2026-10-13 14:05:09 +08:00
PC:        TECH-PC (10.0.0.23) via Wi-Fi
Public:    203.0.113.7 (Example Telecom, Manila)

Idle latency:  17.5 ms (jitter 1.0 ms)
Download:      200.0 Mbps
  under load:  27.0 ms (+9.5 ms, Low)
Upload:        40.0 Mbps
  under load:  27.0 ms (+9.5 ms, Low)
Data used:     300.0 MB

Ratings:
  Web Browsing  *****
  Streaming     *****
  Gaming        *****
  Video Calls   *****

Completed.
"
    );
}

#[tokio::test(start_paused = true)]
async fn a_run_that_found_no_internet_copies_with_nothing_measured_and_says_why() {
    let network = FakeNetwork::new()
        .with_probe_failure(SpeedFailure::Unreachable)
        .with_public_ip(Err("could not reach ipinfo.io".to_string()));

    let text = run(&network).await.to_text();

    assert_eq!(
        text,
        "Speed Test
Server:    (none answered)
Time:      2026-10-13 14:05:09 +08:00
PC:        TECH-PC (10.0.0.23)
Public:    lookup failed: could not reach ipinfo.io

Idle latency:  not measured
Download:      not measured
Upload:        not measured
Data used:     0.0 MB

No internet: the speed test server could not be reached.
"
    );
}

#[tokio::test(start_paused = true)]
async fn a_rate_limited_run_says_how_long_to_wait() {
    let network = FakeNetwork::new().with_probe_failure(SpeedFailure::RateLimited {
        retry_after: Some(Duration::from_secs(30)),
    });

    let text = run(&network).await.to_text();

    assert!(
        text.ends_with(
            "Rate limited: Cloudflare asked to wait 30 s before trying again.
"
        ),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_result_can_be_copied_while_the_run_is_still_going() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    let progress = Progress::default();
    let seen = std::sync::Mutex::new(None);

    speed_test::run_with_progress(
        &network,
        &test,
        &CancellationToken::new(),
        &progress,
        |event| {
            if matches!(event, SpeedTestEvent::Sample(_)) {
                seen.lock()
                    .unwrap()
                    .get_or_insert(progress.read(SpeedTestResult::to_text));
            }
        },
    )
    .await;

    let text = seen
        .lock()
        .unwrap()
        .take()
        .flatten()
        .expect("a Result was readable mid-Run");
    assert!(
        text.contains("Idle latency:  10.0 ms (jitter 0.0 ms)"),
        "{text}"
    );
    assert!(text.contains("Download:      not measured"), "{text}");
    assert!(
        text.ends_with(
            "Still running.
"
        ),
        "{text}"
    );
}

#[tokio::test(start_paused = true)]
async fn the_run_reports_its_phases_the_server_and_the_speed_each_second() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    let mut events = Vec::new();

    speed_test::run(&network, &test, &CancellationToken::new(), |event| {
        events.push(event)
    })
    .await;

    assert_eq!(events[0], SpeedTestEvent::Phase(SpeedPhase::Latency));
    assert!(matches!(&events[1], SpeedTestEvent::Server(server) if server.location == "HKG"));
    assert!(matches!(events[2], SpeedTestEvent::Latency(_)));
    assert_eq!(events[3], SpeedTestEvent::Phase(SpeedPhase::Download));
    let first_sample = events
        .iter()
        .find(|event| matches!(event, SpeedTestEvent::Sample(_)));
    assert_eq!(
        first_sample,
        Some(&SpeedTestEvent::Sample(Sample {
            phase: SpeedPhase::Download,
            at: Duration::from_secs(1),
            bits_per_second: 200_000_000,
        }))
    );
}

#[tokio::test(start_paused = true)]
async fn upload_speed_is_the_total_across_connections_once_the_warm_up_is_over() {
    // Four connections at 10 Mbps each carry 40 Mbps.
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);

    let result = run(&network).await;

    assert_eq!(
        result.upload.expect("upload was measured").bits_per_second,
        40_000_000
    );
    assert!(
        network
            .upload_requests()
            .iter()
            .all(|&bytes| bytes <= 10_000_000)
    );
}

#[tokio::test(start_paused = true)]
async fn an_upload_that_is_stopped_keeps_the_download_and_leaves_the_upload_unmeasured() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    let stop = CancellationToken::new();
    let stopper = stop.clone();
    // Latency takes 0.2 s and the download 10 s, so 15 s is mid-upload.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        stopper.cancel();
    });

    let result = speed_test::run(&network, &test, &stop, |_| {}).await;

    assert_eq!(result.ending, SpeedTestEnding::Stopped);
    assert!(result.download.is_some());
    assert_eq!(result.upload, None);
}

#[test]
fn the_text_lists_the_upload_and_the_star_ratings() {
    let result = measured(200.0, 40.0, 17.5, 1.0);

    let text = result.to_text();

    assert!(text.contains("Upload:        40.0 Mbps\n"), "{text}");
    assert!(
        text.contains(
            "Ratings:\n  Web Browsing  *****\n  Streaming     *****\n  Gaming        *****\n  Video Calls   *****\n"
        ),
        "{text}"
    );
}

/// A finished Result with these figures (Mbps, Mbps, ms, ms), as the real run would build it.
fn measured(download: f64, upload: f64, latency_ms: f64, jitter_ms: f64) -> SpeedTestResult {
    let mbps = |value: f64| Throughput {
        bits_per_second: (value * 1_000_000.0) as u64,
    };
    let millis = |value: f64| Duration::from_micros((value * 1000.0) as u64);
    SpeedTestResult {
        header: toolbox_core::speed_test::SpeedTestHeader {
            started_at: chrono::DateTime::parse_from_rfc3339(common::NOW).unwrap(),
            pc_name: common::PC_NAME.to_string(),
            pc_address: None,
            adapter: None,
            server: None,
            public: None,
        },
        latency: Some(Latency {
            median: millis(latency_ms),
            jitter: millis(jitter_ms),
        }),
        download: Some(mbps(download)),
        upload: Some(mbps(upload)),
        download_loaded: None,
        upload_loaded: None,
        plan: Plan::default(),
        data_used: 0,
        ending: SpeedTestEnding::Completed,
    }
}

fn stars(result: &SpeedTestResult, use_case: UseCase) -> Option<u8> {
    result
        .ratings()
        .into_iter()
        .find(|rating| rating.use_case == use_case)
        .and_then(|rating| rating.stars)
        .map(|Stars(count)| count)
}

#[test]
fn a_fast_steady_line_is_five_stars_everywhere() {
    let result = measured(300.0, 50.0, 12.0, 2.0);
    for use_case in [
        UseCase::WebBrowsing,
        UseCase::Streaming,
        UseCase::Gaming,
        UseCase::VideoCalls,
    ] {
        assert_eq!(stars(&result, use_case), Some(5), "{use_case:?}");
    }
}

#[test]
fn each_use_case_is_only_as_good_as_its_weakest_figure() {
    // 211 Mbps down, 20 Mbps up, 69 ms latency, 20 ms jitter (a real Wi-Fi run).
    let result = measured(211.2, 20.0, 69.0, 20.0);

    // Browsing: latency 69 ms is 4 stars. Streaming: speed 5, jitter 20 ms is 5, so 5.
    // Gaming: latency 69 ms is 3 stars, jitter 20 ms is 4, so 3.
    // Video calls: upload 5, latency 5, jitter 20 ms is 5... latency 69 ms is 5, so 5.
    assert_eq!(stars(&result, UseCase::WebBrowsing), Some(4));
    assert_eq!(stars(&result, UseCase::Streaming), Some(5));
    assert_eq!(stars(&result, UseCase::Gaming), Some(3));
    assert_eq!(stars(&result, UseCase::VideoCalls), Some(5));
}

#[test]
fn a_slow_upload_hurts_video_calls_but_not_browsing() {
    let result = measured(100.0, 1.5, 20.0, 3.0);

    assert_eq!(stars(&result, UseCase::VideoCalls), Some(2));
    assert_eq!(stars(&result, UseCase::WebBrowsing), Some(5));
}

#[test]
fn a_laggy_line_is_one_star_for_gaming() {
    let result = measured(100.0, 20.0, 220.0, 5.0);

    assert_eq!(stars(&result, UseCase::Gaming), Some(1));
}

#[test]
fn an_unmeasured_figure_leaves_the_use_cases_that_need_it_unrated() {
    let mut result = measured(100.0, 20.0, 20.0, 3.0);
    result.upload = None;

    assert_eq!(stars(&result, UseCase::VideoCalls), None);
    assert_eq!(stars(&result, UseCase::Gaming), Some(5));
}

#[tokio::test(start_paused = true)]
async fn latency_under_load_is_measured_for_each_direction_with_its_increase_and_verdict() {
    // Idle 10 ms; 85 ms while downloading (+75, Moderate); 150 ms while uploading (+140, High).
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_loaded_round_trips(ms(85), ms(150));

    let result = run(&network).await;

    assert_eq!(result.download_loaded, Some(ms(85)));
    assert_eq!(result.upload_loaded, Some(ms(150)));
    assert_eq!(
        result.bufferbloat(SpeedDirection::Download),
        Some((ms(75), Bufferbloat::Moderate))
    );
    assert_eq!(
        result.bufferbloat(SpeedDirection::Upload),
        Some((ms(140), Bufferbloat::High))
    );
}

#[test]
fn the_verdict_changes_at_30_and_just_over_100_milliseconds_of_increase() {
    let verdict = |increase_ms: u64| {
        let mut result = measured(100.0, 20.0, 10.0, 1.0);
        result.download_loaded = Some(ms(10 + increase_ms));
        result.bufferbloat(SpeedDirection::Download).unwrap().1
    };

    assert_eq!(verdict(0), Bufferbloat::Low);
    assert_eq!(verdict(29), Bufferbloat::Low);
    assert_eq!(verdict(30), Bufferbloat::Moderate);
    assert_eq!(verdict(100), Bufferbloat::Moderate);
    assert_eq!(verdict(101), Bufferbloat::High);
}

#[test]
fn latency_under_load_that_is_lower_than_idle_is_no_increase() {
    let mut result = measured(100.0, 20.0, 40.0, 1.0);
    result.upload_loaded = Some(ms(35));

    assert_eq!(
        result.bufferbloat(SpeedDirection::Upload),
        Some((Duration::ZERO, Bufferbloat::Low))
    );
}

#[tokio::test(start_paused = true)]
async fn each_loaded_probe_is_reported_as_it_arrives() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10)
        .with_loaded_round_trips(ms(85), ms(150));
    let test = SpeedTest {
        family: AddressFamily::Auto,
        ..SpeedTest::default()
    };
    let mut loaded = Vec::new();

    speed_test::run(&network, &test, &CancellationToken::new(), |event| {
        if let SpeedTestEvent::LoadedProbe(probe) = event {
            loaded.push(probe);
        }
    })
    .await;

    let first = loaded[0];
    assert_eq!(first.direction, SpeedDirection::Download);
    assert_eq!(first.latency, ms(85));
    assert!(
        loaded
            .iter()
            .any(|probe| probe.direction == SpeedDirection::Upload)
    );
}

fn request(download: &str, upload: &str) -> SpeedTestRequest {
    SpeedTestRequest {
        family: AddressFamily::Auto,
        provider: Provider::default(),
        plan_download: download.to_string(),
        plan_upload: upload.to_string(),
    }
}

#[test]
fn plan_speeds_are_optional_and_blank_means_no_comparison() {
    let test = request("", "").validate().expect("blank plans are valid");

    assert_eq!(test.plan, Plan::default());
}

#[test]
fn a_plan_speed_is_read_as_megabits_per_second() {
    let test = request("200", "50.5").validate().expect("valid plans");

    assert_eq!(test.plan.download_bits_per_second, Some(200_000_000));
    assert_eq!(test.plan.upload_bits_per_second, Some(50_500_000));
}

#[test]
fn a_plan_speed_that_is_not_a_positive_number_is_rejected_before_the_run() {
    for bad in ["0", "-5", "fast", "1e400", "NaN"] {
        let error = request(bad, "").validate().expect_err(bad);
        assert!(
            error.message.contains("download"),
            "{bad}: {}",
            error.message
        );
    }
    let error = request("", "abc").validate().expect_err("abc");
    assert!(error.message.contains("upload"), "{}", error.message);
}

#[test]
fn the_result_says_how_much_of_the_plan_each_direction_reached() {
    let mut result = measured(38.0, 60.0, 17.5, 1.0);
    result.plan = Plan {
        download_bits_per_second: Some(200_000_000),
        upload_bits_per_second: Some(50_000_000),
    };

    let text = result.to_text();

    assert_eq!(result.plan_percent(SpeedDirection::Download), Some(19));
    assert_eq!(result.plan_percent(SpeedDirection::Upload), Some(120));
    assert!(
        text.contains("Download:      38.0 Mbps (19% of 200 Mbps plan)\n"),
        "{text}"
    );
    assert!(
        text.contains("Upload:        60.0 Mbps (120% of 50 Mbps plan)\n"),
        "{text}"
    );
}

#[test]
fn a_direction_without_a_plan_speed_has_no_comparison() {
    let mut result = measured(38.0, 60.0, 17.5, 1.0);
    result.plan = Plan {
        download_bits_per_second: Some(200_000_000),
        upload_bits_per_second: None,
    };

    assert_eq!(result.plan_percent(SpeedDirection::Upload), None);
    assert!(result.to_text().contains("Upload:        60.0 Mbps\n"));
}

#[tokio::test(start_paused = true)]
async fn the_plan_the_technician_gave_travels_with_the_result() {
    let network = FakeNetwork::new()
        .with_probes(idle_probes())
        .with_download_rate(50)
        .with_upload_rate(10);
    let test = request("400", "").validate().unwrap();

    let result = speed_test::run(&network, &test, &CancellationToken::new(), |_| {}).await;

    assert_eq!(result.plan_percent(SpeedDirection::Download), Some(50));
}

#[tokio::test(start_paused = true)]
async fn only_a_connection_that_costs_money_needs_confirming_before_a_run() {
    let check = |cost| async move {
        let network = FakeNetwork::new().with_connection_cost(cost);
        speed_test::check_connection(&network).await
    };

    assert_eq!(check(ConnectionCost::Unmetered).await, None);
    assert_eq!(check(ConnectionCost::Unknown).await, None);
    for cost in [
        ConnectionCost::Metered,
        ConnectionCost::Roaming,
        ConnectionCost::OverLimit,
    ] {
        let warning = check(cost).await.expect("needs confirming");
        assert!(warning.contains("data"), "{warning}");
    }
    assert!(
        check(ConnectionCost::Roaming)
            .await
            .unwrap()
            .contains("roaming")
    );
    assert!(
        check(ConnectionCost::OverLimit)
            .await
            .unwrap()
            .contains("limit")
    );
}

#[test]
fn a_long_rate_limit_wait_is_said_in_minutes() {
    let wait = |seconds| SpeedTestEnding::RateLimited {
        retry_after: Some(Duration::from_secs(seconds)),
    };

    assert!(
        wait(90).describe().contains("90 s"),
        "{}",
        wait(90).describe()
    );
    assert!(
        wait(3468).describe().contains("about 58 min"),
        "{}",
        wait(3468).describe()
    );
}

#[test]
fn a_run_that_did_not_complete_is_not_rated() {
    // Latency alone would earn Gaming stars, but the Run never finished.
    let mut result = measured(100.0, 20.0, 20.0, 3.0);
    result.download = None;
    result.upload = None;
    result.ending = SpeedTestEnding::RateLimited { retry_after: None };

    assert!(result.ratings().iter().all(|rating| rating.stars.is_none()));
    assert!(!result.to_text().contains("Ratings:"));
}
