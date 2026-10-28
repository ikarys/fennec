use std::collections::BTreeMap;

use token_usage::{Call, Filter, GroupBy, Ledger, Usage};

fn base_call() -> Call {
    Call {
        external_id: None,
        at_ms: 1,
        product: "product".into(),
        group_id: "group".into(),
        group_name: "Group".into(),
        run_id: "run".into(),
        kind: "turn".into(),
        model: "model".into(),
        usage: Usage {
            prompt_tokens: 10,
            cached_tokens: 2,
            completion_tokens: 3,
        },
        total_secs: 1.0,
        prefill_speed: 1.0,
        generation_speed: 1.0,
        breakdown: BTreeMap::new(),
        savings: BTreeMap::new(),
        labels: BTreeMap::new(),
    }
}

#[test]
fn public_api_summary_totals() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut c1 = base_call();
    c1.at_ms = 1;
    c1.usage = Usage {
        prompt_tokens: 100,
        cached_tokens: 25,
        completion_tokens: 10,
    };
    c1.total_secs = 2.0;
    c1.breakdown.insert("a".into(), 1);
    c1.savings.insert("s".into(), 2);

    let mut c2 = base_call();
    c2.at_ms = 2;
    c2.usage = Usage {
        prompt_tokens: 50,
        cached_tokens: 0,
        completion_tokens: 5,
    };
    c2.total_secs = 3.0;
    c2.breakdown.insert("a".into(), 3);
    c2.breakdown.insert("b".into(), 1);
    c2.savings.insert("s".into(), 4);

    ledger.record(&c1).unwrap();
    ledger.record(&c2).unwrap();

    let summary = ledger.summary(&Filter::default()).unwrap();

    assert_eq!(summary.calls, 2);
    assert_eq!(summary.prompt_tokens, 150);
    assert_eq!(summary.cached_tokens, 25);
    assert_eq!(summary.completion_tokens, 15);
    assert!((summary.cache_hit_rate - 25.0 / 150.0).abs() < 1e-9);
    assert_eq!(summary.total_secs, 5.0);
    assert_eq!(summary.breakdown.get("a"), Some(&4));
    assert_eq!(summary.breakdown.get("b"), Some(&1));
    assert_eq!(summary.savings.get("s"), Some(&6));
}

#[test]
fn public_api_cache_hit_rate_zero_prompt() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut call = base_call();
    call.usage = Usage {
        prompt_tokens: 0,
        cached_tokens: 5,
        completion_tokens: 1,
    };

    ledger.record(&call).unwrap();

    let summary = ledger.summary(&Filter::default()).unwrap();
    assert_eq!(summary.cache_hit_rate, 0.0);
}

#[test]
fn public_api_external_id_dedup() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut call = base_call();
    call.external_id = Some("id1".into());

    assert!(ledger.record(&call).unwrap());
    assert!(!ledger.record(&call).unwrap());

    let summary = ledger.summary(&Filter::default()).unwrap();
    assert_eq!(summary.calls, 1);
}

#[test]
fn public_api_import_inserted_count() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut a = base_call();
    a.external_id = Some("a".into());

    let mut b = base_call();
    b.external_id = Some("b".into());

    let mut a_again = base_call();
    a_again.external_id = Some("a".into());

    let inserted = ledger.import([a, b, a_again]).unwrap();

    assert_eq!(inserted, 2);

    let summary = ledger.summary(&Filter::default()).unwrap();
    assert_eq!(summary.calls, 2);
}

#[test]
fn public_api_filters() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut c1 = base_call();
    c1.group_id = "a".into();
    c1.at_ms = 100;

    let mut c2 = base_call();
    c2.group_id = "b".into();
    c2.at_ms = 200;

    let mut c3 = base_call();
    c3.group_id = "a".into();
    c3.at_ms = 300;

    let mut c4 = base_call();
    c4.at_ms = 400;
    c4.labels.insert("env".into(), "prod".into());

    ledger.record(&c1).unwrap();
    ledger.record(&c2).unwrap();
    ledger.record(&c3).unwrap();
    ledger.record(&c4).unwrap();

    let group_a = Filter {
        group_id: Some("a".into()),
        ..Default::default()
    };
    assert_eq!(ledger.summary(&group_a).unwrap().calls, 2);

    let window = Filter {
        since_ms: Some(150),
        until_ms: Some(300),
        ..Default::default()
    };
    assert_eq!(ledger.summary(&window).unwrap().calls, 1);

    let labeled = Filter {
        label: Some(("env".into(), "prod".into())),
        ..Default::default()
    };
    assert_eq!(ledger.summary(&labeled).unwrap().calls, 1);
}

#[test]
fn public_api_series_day_utc_midnight() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut before = base_call();
    before.at_ms = 1_704_153_599_999;

    let mut after = base_call();
    after.at_ms = 1_704_153_600_000;

    ledger.record(&before).unwrap();
    ledger.record(&after).unwrap();

    let rows = ledger.series(&Filter::default(), GroupBy::Day).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key, "2024-01-01");
    assert_eq!(rows[1].key, "2024-01-02");
}

#[test]
fn public_api_series_group_latest_name() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut old = base_call();
    old.at_ms = 1;
    old.group_id = "g".into();
    old.group_name = "old".into();

    let mut new = base_call();
    new.at_ms = 2;
    new.group_id = "g".into();
    new.group_name = "new".into();

    ledger.record(&old).unwrap();
    ledger.record(&new).unwrap();

    let rows = ledger.series(&Filter::default(), GroupBy::Group).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "g");
    assert_eq!(rows[0].name, "new");
}

#[test]
fn public_api_breakdown_savings_summed() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut c1 = base_call();
    c1.model = "m".into();
    c1.breakdown.insert("a".into(), 1);
    c1.savings.insert("s".into(), 2);

    let mut c2 = base_call();
    c2.model = "m".into();
    c2.breakdown.insert("a".into(), 3);
    c2.savings.insert("s".into(), 4);

    ledger.record(&c1).unwrap();
    ledger.record(&c2).unwrap();

    let rows = ledger.series(&Filter::default(), GroupBy::Model).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "m");
    assert_eq!(rows[0].summary.breakdown.get("a"), Some(&4));
    assert_eq!(rows[0].summary.savings.get("s"), Some(&6));
}

#[test]
fn public_api_open_file_twice_keeps_data() {
    let path =
        std::env::temp_dir().join(format!("usage-public-api-file-{}.db", std::process::id()));

    let _ = std::fs::remove_file(&path);

    let ledger = Ledger::open(&path).unwrap();
    ledger.record(&base_call()).unwrap();
    drop(ledger);

    let ledger = Ledger::open(&path).unwrap();
    let summary = ledger.summary(&Filter::default()).unwrap();
    assert_eq!(summary.calls, 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn public_api_groups_independent() {
    let ledger = Ledger::open_in_memory().unwrap();

    let mut a = base_call();
    a.group_id = "a".into();

    let mut b = base_call();
    b.group_id = "b".into();

    ledger.record(&a).unwrap();
    ledger.record(&b).unwrap();

    let filter_a = Filter {
        group_id: Some("a".into()),
        ..Default::default()
    };
    let filter_b = Filter {
        group_id: Some("b".into()),
        ..Default::default()
    };

    assert_eq!(ledger.summary(&filter_a).unwrap().calls, 1);
    assert_eq!(ledger.summary(&filter_b).unwrap().calls, 1);
}
