use super::*;
use crate::domain::mcp::McpTool;

#[test]
fn preferences_stored_before_shortcuts_load_with_none() {
    assert!(
        serde_json::from_str::<Preferences>("{}")
            .unwrap()
            .shortcuts
            .is_empty()
    );
}

#[test]
fn preferences_validate_rejects_bad_shortcuts() {
    let mut prefs = serde_json::from_str::<Preferences>("{}").unwrap();
    prefs.shortcuts.insert("palette".into(), "KeyK".into());
    assert!(prefs.validate().is_err());
}

#[test]
fn search_prefs_stored_before_web_fetch_allow_it() {
    let search: SearchPrefs = serde_json::from_str(r#"{"searxngUrl":"http://s"}"#).unwrap();
    assert!(search.web_fetch);
}

#[test]
fn preferences_stored_before_delegation_allow_it() {
    let prefs: Preferences = serde_json::from_str("{}").unwrap();
    assert!(prefs.delegation.enabled);
    assert_eq!(prefs.delegation.max_per_turn, 5);
}

#[test]
fn preferences_stored_before_model_trailers_keep_them_on() {
    let prefs: Preferences = serde_json::from_str("{}").unwrap();
    assert!(prefs.integration.model_trailers);
}

fn defaults() -> ModelParams {
    ModelParams {
        provider: LOCAL_PROVIDER.into(),
        model: "default".into(),
        max_tokens: 32_000,
        context_window: 128_000,
        compact_at: 0.85,
        reasoning_effort: None,
    }
}

fn local_provider(model: &str, prefs: ModelPrefs) -> Provider {
    let mut models = BTreeMap::new();
    models.insert(model.to_string(), prefs);
    Provider {
        id: LOCAL_PROVIDER.to_string(),
        name: String::new(),
        base_url: String::new(),
        api_key: None,
        models,
    }
}

fn valid() -> Preferences {
    Preferences {
        selected: Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
        }),
        providers: vec![local_provider(
            "m",
            ModelPrefs {
                price: None,
                max_tokens: Some(100),
                context_window: Some(2048),
                compaction_threshold: Some(0.5),
                reasoning_effort: Some(ReasoningEffort::High),
                reasoning_efforts: Vec::new(),
            },
        )],
        ..Default::default()
    }
}

fn local_model<'a>(prefs: &'a mut Preferences, model: &str) -> &'a mut ModelPrefs {
    let local = prefs
        .providers
        .iter_mut()
        .find(|p| p.id == LOCAL_PROVIDER)
        .unwrap();
    local.models.get_mut(model).unwrap()
}

#[test]
fn empty_preferences_resolve_to_the_defaults() {
    let resolved = Preferences::default().resolve(None, &defaults(), |_| None);
    assert_eq!(resolved, defaults());
}

#[test]
fn per_model_preferences_override_the_defaults() {
    let resolved = valid().resolve(None, &defaults(), |_| None);
    assert_eq!(
        resolved,
        ModelParams {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
            max_tokens: 100,
            context_window: 2048,
            compact_at: 0.5,
            reasoning_effort: Some(ReasoningEffort::High),
        }
    );
}

#[test]
fn a_reasoning_effort_resolves_per_model() {
    let mut p = valid();
    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "a".into(),
    });
    p.providers[0].models.insert(
        "a".into(),
        ModelPrefs {
            reasoning_effort: Some(ReasoningEffort::Minimal),
            reasoning_efforts: Vec::new(),
            ..Default::default()
        },
    );
    assert_eq!(
        p.resolve(None, &defaults(), |_| None).reasoning_effort,
        Some(ReasoningEffort::Minimal)
    );

    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "b".into(),
    });
    assert_eq!(
        p.resolve(None, &defaults(), |_| None).reasoning_effort,
        None
    );

    let with_default = ModelParams {
        reasoning_effort: Some(ReasoningEffort::Low),
        ..defaults()
    };
    assert_eq!(
        p.resolve(None, &with_default, |_| None).reasoning_effort,
        Some(ReasoningEffort::Low)
    );
}

#[test]
fn a_model_preference_does_not_leak_to_another_model() {
    let mut p = Preferences::default();
    p.providers.push(local_provider(
        "a",
        ModelPrefs {
            price: None,
            max_tokens: Some(100),
            context_window: Some(2048),
            compaction_threshold: Some(0.5),
            reasoning_effort: None,
            reasoning_efforts: Vec::new(),
        },
    ));

    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "a".into(),
    });
    let a = p.resolve(None, &defaults(), |_| None);
    assert_eq!(a.max_tokens, 100);
    assert_eq!(a.context_window, 2048);
    assert_eq!(a.compact_at, 0.5);

    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "b".into(),
    });
    let b = p.resolve(None, &defaults(), |_| None);
    assert_eq!(b.max_tokens, 32_000);
    assert_eq!(b.context_window, 128_000);
    assert_eq!(b.compact_at, 0.85);
}

#[test]
fn a_model_without_preferences_uses_the_listed_window() {
    let p = Preferences {
        selected: Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "listed".into(),
        }),
        ..Default::default()
    };
    let resolved = p.resolve(None, &defaults(), |ref_| {
        (ref_.provider == LOCAL_PROVIDER && ref_.model == "listed").then_some(64_000)
    });
    assert_eq!(
        resolved,
        ModelParams {
            provider: LOCAL_PROVIDER.to_string(),
            model: "listed".into(),
            max_tokens: 32_000,
            context_window: 64_000,
            compact_at: 0.85,
            reasoning_effort: None,
        }
    );
}

#[test]
fn a_chosen_model_overrides_the_selected_one() {
    let mut p = valid();
    p.providers[0].models.insert(
        "other".into(),
        ModelPrefs {
            price: None,
            max_tokens: Some(200),
            context_window: Some(4096),
            compaction_threshold: Some(0.75),
            reasoning_effort: None,
            reasoning_efforts: Vec::new(),
        },
    );
    let chosen = ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "other".into(),
    };
    let resolved = p.resolve(Some(&chosen), &defaults(), |_| None);
    assert_eq!(
        resolved,
        ModelParams {
            provider: LOCAL_PROVIDER.to_string(),
            model: "other".into(),
            max_tokens: 200,
            context_window: 4096,
            compact_at: 0.75,
            reasoning_effort: None,
        }
    );
}

#[test]
fn an_empty_chosen_model_falls_back_to_the_selected_one() {
    let p = valid();
    let chosen = ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: String::new(),
    };
    let resolved = p.resolve(Some(&chosen), &defaults(), |_| None);
    assert_eq!(resolved.model, "m");
}

#[test]
fn validate_rejects_bad_fields() {
    let mut p = valid();
    local_model(&mut p, "m").max_tokens = Some(0);
    let err = p.validate().unwrap_err();
    assert!(err.contains("m"));
    assert!(err.contains("max_tokens"));

    let mut p = valid();
    local_model(&mut p, "m").context_window = Some(1023);
    let err = p.validate().unwrap_err();
    assert!(err.contains("context_window"));

    let mut p = valid();
    local_model(&mut p, "m").compaction_threshold = Some(1.0001);
    let err = p.validate().unwrap_err();
    assert!(err.contains("compaction_threshold"));

    let mut p = valid();
    local_model(&mut p, "m").compaction_threshold = Some(f64::NAN);
    assert!(p.validate().is_err());

    let mut p = valid();
    p.providers[0]
        .models
        .insert(String::new(), ModelPrefs::default());
    assert!(p.validate().unwrap_err().contains("model id"));

    let mut p = valid();
    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: String::new(),
    });
    assert!(p.validate().unwrap_err().contains("model"));

    let mut p = valid();
    p.selected = Some(ModelRef {
        provider: "missing".into(),
        model: "m".into(),
    });
    assert!(p.validate().unwrap_err().contains("unknown provider"));

    let mut p = valid();
    p.providers.push(Provider {
        id: "other".into(),
        name: "Other".into(),
        base_url: "ftp://example.com".into(),
        api_key: None,
        models: BTreeMap::new(),
    });
    assert!(p.validate().unwrap_err().contains("base_url"));

    let mut p = valid();
    p.providers[0].base_url = "ftp://example.com".into();
    assert!(p.validate().unwrap_err().contains("base_url"));

    let mut p = valid();
    p.providers.push(Provider {
        id: LOCAL_PROVIDER.to_string(),
        name: "Local again".into(),
        base_url: String::new(),
        api_key: None,
        models: BTreeMap::new(),
    });
    assert!(p.validate().unwrap_err().contains("duplicate provider"));

    let mut p = valid();
    p.display.font_size = 12;
    assert!(p.validate().unwrap_err().contains("font_size"));

    let mut p = valid();
    p.display.font_size = 17;
    assert!(p.validate().unwrap_err().contains("font_size"));

    let mut p = valid();
    p.search.searxng_url = Some("ftp://example.com".into());
    assert!(p.validate().unwrap_err().contains("searxng_url"));
}

#[test]
fn a_provider_model_is_the_default_when_nothing_else_names_one() {
    let mut p = valid();
    p.selected = None;
    p.providers[0].models.clear();
    p.providers[0]
        .models
        .insert("listed".into(), ModelPrefs::default());
    let no_environment = ModelParams {
        model: String::new(),
        ..defaults()
    };

    let chosen = p.default_model(&no_environment);
    assert_eq!(chosen.model, "listed");
    assert_eq!(chosen.provider, p.providers[0].id);

    assert_eq!(p.default_model(&defaults()).model, defaults().model);
}

#[test]
fn orphan_favorites_are_forgotten() {
    let mut p = valid();
    let kept = ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "m".into(),
    };
    p.favorites = vec![
        kept.clone(),
        ModelRef {
            provider: "removed".into(),
            model: "m".into(),
        },
    ];
    p.forget_orphan_favorites();
    assert_eq!(p.favorites, vec![kept]);
    assert!(p.validate().is_ok());
}

#[test]
fn validate_rejects_unknown_and_duplicate_favorites() {
    let mut p = valid();
    p.favorites.push(ModelRef {
        provider: "missing".into(),
        model: "m".into(),
    });
    assert!(p.validate().unwrap_err().contains("unknown provider"));

    let mut p = valid();
    p.favorites.push(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "m".into(),
    });
    p.favorites.push(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "m".into(),
    });
    assert!(p.validate().unwrap_err().contains("duplicate favorite"));
}

#[test]
fn validate_rejects_duplicate_mcp_server_names() {
    let mut p = valid();
    p.mcp = vec![
        McpServer {
            name: "fs".into(),
            command: Some("fs-mcp".into()),
            ..Default::default()
        },
        McpServer {
            name: "fs".into(),
            command: Some("fs-mcp".into()),
            ..Default::default()
        },
    ];
    assert!(p.validate().unwrap_err().contains("duplicate mcp server"));
}

#[test]
fn validate_rejects_an_mcp_tool_that_collides_with_a_builtin() {
    let mut p = valid();
    p.mcp = vec![McpServer {
        name: "read".into(),
        command: Some("read-mcp".into()),
        cached_tools: vec![McpTool {
            annotations: None,
            name: "file".into(),
            description: None,
            input_schema: serde_json::json!({}),
        }],
        ..Default::default()
    }];
    assert!(
        p.validate()
            .unwrap_err()
            .contains("collides with a built-in tool")
    );
}

#[test]
fn validate_accepts_a_well_formed_mcp_server() {
    let mut p = valid();
    p.mcp = vec![McpServer {
        name: "fs".into(),
        command: Some("fs-mcp".into()),
        ..Default::default()
    }];
    assert!(p.validate().is_ok());
}

#[test]
fn preferences_round_trip_and_empty_json_loads() {
    let mut models = BTreeMap::new();
    models.insert(
        "m".into(),
        ModelPrefs {
            price: None,
            max_tokens: Some(100),
            context_window: Some(2048),
            compaction_threshold: Some(0.25),
            reasoning_effort: Some(ReasoningEffort::XHigh),
            reasoning_efforts: Vec::new(),
        },
    );
    let mut p = Preferences {
        selected: Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "m".into(),
        }),
        providers: vec![Provider {
            id: LOCAL_PROVIDER.to_string(),
            name: "Local".into(),
            base_url: String::new(),
            api_key: Some(ApiKey::Env("FENNEC_API_KEY".into())),
            models,
        }],
        ..Default::default()
    };
    p.display.reasoning_open = false;
    p.display.font_size = 15;
    p.display.language = Lang::Fr;
    p.global_instructions = Some("Be terse.".into());
    p.sandbox = SandboxPrefs {
        enabled: false,
        network: true,
    };

    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"providers\""));
    assert!(json.contains("\"selected\""));
    assert!(json.contains("\"maxTokens\""));
    assert!(json.contains("\"reasoningEffort\""));
    assert!(json.contains("\"fontSize\""));
    assert!(json.contains("\"language\""));
    assert!(json.contains("\"globalInstructions\""));
    assert!(json.contains("\"sandbox\""));
    assert_eq!(serde_json::from_str::<Preferences>(&json).unwrap(), p);
    assert_eq!(
        serde_json::from_str::<Preferences>("{}").unwrap(),
        Preferences::default()
    );
}

#[test]
fn a_preferences_json_without_the_new_fields_loads() {
    let mut p: Preferences = serde_json::from_str(
        r#"{"models":{"m":{"maxTokens":100}},"display":{"reasoningOpen":true,"toolResultsOpen":false}}"#,
    )
    .unwrap();
    p.migrate();
    assert_eq!(p.display.font_size, DEFAULT_FONT_SIZE);
    assert_eq!(p.display.language, Lang::En);
    assert!(p.display.sounds);
    assert_eq!(
        p.provider(LOCAL_PROVIDER)
            .unwrap()
            .models
            .get("m")
            .unwrap()
            .reasoning_effort,
        None
    );
    assert_eq!(p.sandbox, SandboxPrefs::default());
    assert!(p.sandbox.enabled);
    assert!(!p.sandbox.network);
    assert!(p.validate().is_ok());
}

/// The wire model id and the parameters of a session from before providers
/// must not change, or its prompt cache is lost.
#[test]
fn legacy_preferences_resolve_to_the_same_parameters() {
    let mut p: Preferences = serde_json::from_str(
        r#"{"model":"qwen3.8-27b","models":{"qwen3.8-27b":{"maxTokens":100,"contextWindow":2048,"compactionThreshold":0.5,"reasoningEffort":"high"}}}"#,
    )
    .unwrap();
    p.migrate();
    assert_eq!(
        p.resolve(None, &defaults(), |_| None),
        ModelParams {
            provider: LOCAL_PROVIDER.to_string(),
            model: "qwen3.8-27b".into(),
            max_tokens: 100,
            context_window: 2048,
            compact_at: 0.5,
            reasoning_effort: Some(ReasoningEffort::High),
        }
    );
}

#[test]
fn legacy_preferences_without_a_model_resolve_to_the_defaults() {
    for json in ["{}", r#"{"models":{}}"#] {
        let mut p: Preferences = serde_json::from_str(json).unwrap();
        p.migrate();
        assert_eq!(p.resolve(None, &defaults(), |_| None), defaults());
    }
}

#[test]
fn migration_is_idempotent_and_writes_no_legacy_field() {
    let mut p: Preferences =
        serde_json::from_str(r#"{"model":"m","models":{"m":{"maxTokens":100}}}"#).unwrap();
    p.migrate();
    let once = p.clone();
    p.migrate();
    assert_eq!(p, once);

    let value = serde_json::to_value(&p).unwrap();
    assert!(value.get("model").is_none());
    assert!(value.get("models").is_none());

    let mut reloaded: Preferences = serde_json::from_value(value).unwrap();
    reloaded.migrate();
    assert_eq!(reloaded, once);
}

#[test]
fn migration_keeps_newer_values() {
    let mut p: Preferences =
        serde_json::from_str(r#"{"model":"old","models":{"old":{"maxTokens":1}}}"#).unwrap();
    p.selected = Some(ModelRef {
        provider: LOCAL_PROVIDER.to_string(),
        model: "new".into(),
    });

    let mut models = BTreeMap::new();
    models.insert(
        "new".into(),
        ModelPrefs {
            max_tokens: Some(2),
            reasoning_efforts: Vec::new(),
            ..Default::default()
        },
    );
    p.providers.push(Provider {
        id: LOCAL_PROVIDER.to_string(),
        name: String::new(),
        base_url: String::new(),
        api_key: None,
        models,
    });

    p.migrate();
    assert_eq!(
        p.selected,
        Some(ModelRef {
            provider: LOCAL_PROVIDER.to_string(),
            model: "new".into()
        })
    );
    let local = p.provider(LOCAL_PROVIDER).unwrap();
    assert_eq!(local.models.get("new").unwrap().max_tokens, Some(2));
    assert_eq!(local.models.get("old").unwrap().max_tokens, Some(1));
}

#[test]
fn the_listed_window_is_asked_for_the_selected_provider_and_model() {
    let p = Preferences {
        selected: Some(ModelRef {
            provider: "other".into(),
            model: "m".into(),
        }),
        ..Default::default()
    };
    let resolved = p.resolve(None, &defaults(), |selected| {
        (selected.provider == "other" && selected.model == "m").then_some(64_000)
    });
    assert_eq!(resolved.context_window, 64_000);
}

#[test]
fn seal_keys_seals_only_plain_keys() {
    let mut p = Preferences::default();
    p.providers.push(Provider {
        id: "a".into(),
        name: "A".into(),
        base_url: "http://a".into(),
        api_key: Some(ApiKey::Plain("plain".into())),
        models: BTreeMap::new(),
    });
    p.providers.push(Provider {
        id: "b".into(),
        name: "B".into(),
        base_url: "http://b".into(),
        api_key: Some(ApiKey::Env("B_KEY".into())),
        models: BTreeMap::new(),
    });

    assert!(p.has_plain_keys());
    p.seal_keys(|key| format!("sealed:{key}"));
    assert!(!p.has_plain_keys());
    assert_eq!(
        p.providers[0].api_key,
        Some(ApiKey::Sealed("sealed:plain".into()))
    );
    assert_eq!(p.providers[1].api_key, Some(ApiKey::Env("B_KEY".into())));
}

#[test]
fn an_old_stored_tariff_is_ignored() {
    let p: Preferences = serde_json::from_str(r#"{"tariff":{"inputPerM":1}}"#).unwrap();
    assert!(p.providers.is_empty());
}
