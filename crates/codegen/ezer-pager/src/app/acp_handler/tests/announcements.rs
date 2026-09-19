#![cfg_attr(rustfmt, rustfmt::skip)]
    use super::*;

    /// Stale or duplicate gens short-circuit BEFORE the apply (and its config disk loads) runs; nothing observable may change.
    #[test]
    fn announcements_update_stale_gen_short_circuits_before_apply() {
        let mut app = make_app_with_agent("sess-ann");
        app.announcements_last_gen = 5;
        app.active_announcements = vec![critical_announcement("current")];
        // This marker cannot survive an apply: it matches no pushed announcement, so any apply would prune it (and queue a persist)
        app.hidden_announcement_ids = ["stale-key".to_string()].into_iter().collect();

        for stale_gen in [4, 5] {
            let changed = handle_ext_notification(
                &announcements_update_notif(stale_gen, &[critical_announcement("stale-push")]),
                &mut app,
            );
            assert!(!changed, "gen {stale_gen} must be dropped at the watermark");
        }

        assert_eq!(
            app.active_announcements,
            vec![critical_announcement("current")]
        );
        assert_eq!(app.announcements_last_gen, 5);
        assert!(app.hidden_announcement_ids.contains("stale-key"));
        assert!(
            !app.pending_effects
                .iter()
                .any(|e| matches!(e, Effect::PersistAnnouncementsHidden { .. })),
            "short-circuit must not reach prune/persist, got {:?}",
            app.pending_effects
        );
    }

    /// The watermark lasts one connection: the event loop resets it to 0 on leader reconnect.
    /// A re-elected shell's fresh (possibly lower) gen sequence then applies, but xAI remote
    /// payloads are ignored so leftover grok.com credentials cannot reintroduce banners.
    #[test]
    fn announcements_update_applies_after_reconnect_watermark_reset() {
        let mut app = make_app_with_agent("sess-ann");
        // The previous connection left a watermark ahead of the new shell's gens
        app.announcements_last_gen = 9_999_999_999;
        // The event loop's leader-reconnected branch does this reset
        app.announcements_last_gen = 0;

        let first = handle_ext_notification(
            &announcements_update_notif(1, &[critical_announcement("fresh")]),
            &mut app,
        );
        assert!(first, "gen 1 must apply after the reconnect reset");
        assert_eq!(app.announcements_last_gen, 1);
        assert!(
            app.active_announcements
                .iter()
                .all(|a| a.id.as_deref() != Some("fresh")),
            "BYOK: xAI-pushed announcements must not land"
        );

        // The per-client seed broadcast can deliver the same gen twice.
        let dup = handle_ext_notification(
            &announcements_update_notif(1, &[critical_announcement("fresh")]),
            &mut app,
        );
        assert!(!dup, "duplicate seed copy must be idempotent");
        assert_eq!(app.announcements_last_gen, 1);
    }

    /// Remote xAI lists are ignored, so a push with only remote ids prunes hidden keys
    /// that no longer match a local/managed announcement.
    #[test]
    fn announcements_update_prunes_stale_hidden_ids_and_persists() {
        let mut app = make_app_with_agent("sess-ann");
        app.hidden_announcement_ids = ["gone".to_string(), "live".to_string()]
            .into_iter()
            .collect();

        apply_announcements_update(
            &mut app,
            1,
            &[critical_announcement("live")],
            None,
            None,
            None,
        );

        assert_eq!(app.announcements_last_gen, 1);
        assert!(
            app.hidden_announcement_ids.is_empty(),
            "remote-only ids must not keep hide keys alive"
        );
        let expected: std::collections::BTreeSet<String> = Default::default();
        assert!(
            app.pending_effects.iter().any(|e| matches!(
                e,
                Effect::PersistAnnouncementsHidden { hidden_ids } if hidden_ids == &expected
            )),
            "prune must persist the shrunken set, got {:?}",
            app.pending_effects
        );
        assert_eq!(shown_banner_id(&app), None);
        assert!(
            app.active_announcements.is_empty(),
            "BYOK: remote xAI announcements must not become active"
        );
    }

    /// A pushed xAI critical must never re-show a banner, even with a new id.
    #[test]
    fn announcements_update_ignores_remote_xai_payload() {
        let mut app = make_app_with_agent("sess-ann");
        apply_announcements_update(
            &mut app,
            1,
            &[critical_announcement("outage-a")],
            None,
            None,
            None,
        );
        assert_eq!(shown_banner_id(&app), None);
        assert!(app.active_announcements.is_empty());

        apply_announcements_update(
            &mut app,
            2,
            &[critical_announcement("outage-b")],
            None,
            None,
            None,
        );

        assert_eq!(app.announcements_last_gen, 2);
        assert_eq!(
            shown_banner_id(&app),
            None,
            "BYOK: xAI announcement payloads must not rearm banners"
        );
        assert!(app.active_announcements.is_empty());
    }

    /// A push must not drop config-layer announcements, and prune must not erase their persisted hide keys.
    /// Config layers re-resolve every launch, so a dropped key would re-show a critical the user already hid.
    /// Remote xAI lists are ignored even when they arrive on the same push.
    #[test]
    fn announcements_update_remerges_config_layers_and_keeps_their_hide_keys() {
        let mut app = make_app_with_agent("sess-ann");
        let user_cfg: toml::Value = toml::from_str(
            r#"
            [[announcements]]
            id = "cfg-crit"
            title = "Config outage"
            message = "from user config"
            severity = "critical"
            "#,
        )
        .unwrap();
        app.hidden_announcement_ids = ["cfg-crit".to_string()].into_iter().collect();

        apply_announcements_update(
            &mut app,
            1,
            &[critical_announcement("live")],
            None,
            Some(&user_cfg),
            None,
        );

        assert_eq!(app.announcements_last_gen, 1);
        let ids: Vec<_> = app
            .active_announcements
            .iter()
            .filter_map(|a| a.id.as_deref())
            .collect();
        assert_eq!(
            ids,
            ["cfg-crit"],
            "config-layer announcement must survive; xAI remote list must not merge in"
        );
        assert!(
            app.hidden_announcement_ids.contains("cfg-crit"),
            "config-layer hide key must survive prune"
        );
        assert!(
            !app.pending_effects
                .iter()
                .any(|e| matches!(e, Effect::PersistAnnouncementsHidden { .. })),
            "unchanged hidden set must not schedule a persist, got {:?}",
            app.pending_effects
        );
        assert_eq!(
            shown_banner_id(&app),
            None,
            "hidden config-layer announcement stays skipped; remote xAI banner must not show"
        );
    }

    /// A mid-session push of local/managed announcements must open the `/announcements` gate
    /// on already-live subagent child views, not just top-level agents. Remote xAI lists do not.
    #[test]
    fn announcements_update_fans_slash_gate_to_live_subagent_views() {
        let mut app = make_app_with_parent_and_child("parent-sess", "child-sess");
        assert!(
            !test_subagent(test_agent(&app, AgentId(0)), "child-sess")
                .prompt
                .slash_controller
                .has_session_announcements(),
            "gate starts closed"
        );

        apply_announcements_update(
            &mut app,
            1,
            &[critical_announcement("outage-a")],
            None,
            None,
            None,
        );

        let agent = test_agent(&app, AgentId(0));
        assert!(
            !agent.prompt.slash_controller.has_session_announcements(),
            "remote-only xAI push must not open the parent gate"
        );
        assert!(
            !test_subagent(agent, "child-sess")
                .prompt
                .slash_controller
                .has_session_announcements(),
            "remote-only xAI push must not open the child gate"
        );

        let user_cfg: toml::Value = toml::from_str(
            r#"
            [[announcements]]
            id = "cfg-outage"
            title = "Local outage"
            message = "from user config"
            severity = "critical"
            "#,
        )
        .unwrap();
        apply_announcements_update(
            &mut app,
            2,
            &[critical_announcement("outage-a")],
            None,
            Some(&user_cfg),
            None,
        );

        let agent = test_agent(&app, AgentId(0));
        assert!(
            agent.prompt.slash_controller.has_session_announcements(),
            "parent gate open from local config-layer announcement"
        );
        assert!(
            test_subagent(agent, "child-sess")
                .prompt
                .slash_controller
                .has_session_announcements(),
            "live child view gate open from local config-layer announcement"
        );
    }
