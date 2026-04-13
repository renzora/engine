use std::hint::black_box;

use bevy::{ecs::system::RunSystemOnce, prelude::*};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use bevy_gauge::prelude::*;
use bevy_gauge::attribute_id::Interner;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AttributesPlugin);
    app
}

fn setup_app_with_entity() -> (App, Entity) {
    let mut app = setup_app();
    let entity = app.world_mut().spawn(Attributes::new()).id();
    (app, entity)
}

fn setup_app_with_entities(count: usize) -> (App, Vec<Entity>) {
    let mut app = setup_app();
    let entities = (0..count)
        .map(|_| app.world_mut().spawn(Attributes::new()).id())
        .collect();
    (app, entities)
}

// ---------------------------------------------------------------------------
// 1. Stat access — cached read via &Attributes
// ---------------------------------------------------------------------------

pub fn bench_stat_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("stat_access");

    for value in [10.0, 100.0, 1000.0] {
        group.bench_with_input(
            BenchmarkId::from_parameter(value as u32),
            &value,
            |b, &value| {
                let (mut app, entity) = setup_app_with_entity();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(entity, "Life.base", value);
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    let attrs = app.world().get::<Attributes>(entity).unwrap();
                    black_box(attrs.value("Life.base"));
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 2. Intra-entity dependency chains
// ---------------------------------------------------------------------------

pub fn bench_dependent_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependent_stats_intra_entity");

    for chain_length in [1, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(chain_length),
            &chain_length,
            |b, &cl| {
                let (mut app, entity) = setup_app_with_entity();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(entity, "Base", 10.0);
                        for i in 1..=cl {
                            let prev = if i == 1 {
                                "Base".to_string()
                            } else {
                                format!("Level{}", i - 1)
                            };
                            let curr = format!("Level{}", i);
                            let _ = stats.add_expr_modifier(
                                entity,
                                &curr,
                                &format!("{prev} * 1.1"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                let final_stat = format!("Level{cl}");
                b.iter(|| {
                    let s = final_stat.clone();
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(entity, &s));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 3. Inter-entity dependency chains
// ---------------------------------------------------------------------------

pub fn bench_entity_dependencies(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependent_stats_inter_entity");

    for chain_length in [1, 3, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(chain_length),
            &chain_length,
            |b, &cl| {
                let (mut app, entities) = setup_app_with_entities(cl + 1);
                let last = entities[cl];
                let ents = entities.clone();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(ents[0], "Power.base", 100.0);
                        for i in 1..=cl {
                            stats.register_source(ents[i], "Source", ents[i - 1]);
                            let _ = stats.add_expr_modifier(
                                ents[i],
                                "Power.base",
                                "Power.base@Source * 0.9",
                            );
                        }
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(last, "Power.base"));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 4. Tagged stat evaluation
// ---------------------------------------------------------------------------

pub fn bench_tag_based_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("tag_based_stats_evaluation");

    for tag_count in [1, 3, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(tag_count),
            &tag_count,
            |b, &tc| {
                let (mut app, entity) = setup_app_with_entity();

                // Register tag names
                {
                    let mut resolver = app.world_mut().resource_mut::<TagResolver>();
                    for i in 0..tc {
                        resolver.register(&format!("TAG{i}"), TagMask::bit(i as u32));
                    }
                }

                let el = entity;
                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats
                            .tagged_attribute(
                                el,
                                "Damage",
                                &[("base", ReduceFn::Sum), ("increased", ReduceFn::Sum)],
                                "base * (1 + increased)",
                            )
                            .unwrap();

                        stats.add_modifier(el, "Damage.base", 10.0);
                        for i in 0..tc {
                            let tag = TagMask::bit(i as u32);
                            stats.add_modifier_tagged(
                                el,
                                "Damage.base",
                                5.0 + i as f32,
                                tag,
                            );
                            stats.add_modifier_tagged(
                                el,
                                "Damage.increased",
                                0.1 * (i as f32 + 1.0),
                                tag,
                            );
                        }
                    })
                    .unwrap();
                app.update();

                let first_tag = TagMask::bit(0);
                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate_tagged(el, "Damage", first_tag));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 5. Mixed cross-entity + local dependencies
// ---------------------------------------------------------------------------

pub fn bench_mixed_dependencies(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_dependencies_evaluation");

    for complexity in [1, 3, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(complexity),
            &complexity,
            |b, &compl| {
                let (mut app, entities) = setup_app_with_entities(compl + 1);
                let last = entities[compl];
                let ents = entities.clone();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(ents[0], "Power.base", 20.0);
                        for i in 1..=compl {
                            stats.register_source(ents[i], "Source", ents[0]);
                            stats.add_modifier(
                                ents[i],
                                "Multiplier.base",
                                1.0 + (i as f32 * 0.1),
                            );
                            let _ = stats.add_expr_modifier(
                                ents[i],
                                "Damage.base",
                                "Power.base@Source * Multiplier.base",
                            );
                            if i > 1 {
                                stats.register_source(ents[i], "Prev", ents[i - 1]);
                                let _ = stats.add_expr_modifier(
                                    ents[i],
                                    "ComplexDamage.base",
                                    "(Power.base@Source * 0.5) + (Damage.base@Prev * 0.3) * Multiplier.base",
                                );
                            }
                        }
                    })
                    .unwrap();
                app.update();

                let stat = if compl > 1 {
                    "ComplexDamage.base"
                } else {
                    "Damage.base"
                };
                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(last, stat));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 6. Update propagation — central entity → N dependents
// ---------------------------------------------------------------------------

pub fn bench_stats_update_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("stats_update_propagation");

    for entity_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |b, &ec| {
                let (mut app, entities) = setup_app_with_entities(ec + 1);
                let central = entities[0];
                let dependents: Vec<Entity> = entities[1..].to_vec();
                let deps = dependents.clone();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(central, "Aura.base", 10.0);
                        for (i, &dep) in deps.iter().enumerate() {
                            stats.register_source(dep, "CentralSource", central);
                            let mult = 0.8 + ((i as f32 % 5.0) * 0.1);
                            let _ = stats.add_expr_modifier(
                                dep,
                                "Buff.value",
                                &format!("Aura.base@CentralSource * {mult}"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                let deps_read = dependents.clone();
                b.iter(|| {
                    // Mutate the central entity — propagation happens synchronously
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            stats.add_modifier(central, "Aura.base", 1.0);
                        })
                        .unwrap();
                    app.update();

                    // Read all dependent values
                    let deps_inner = deps_read.clone();
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            for &dep in &deps_inner {
                                black_box(stats.evaluate(dep, "Buff.value"));
                            }
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 7. Expression complexity
// ---------------------------------------------------------------------------

pub fn bench_complex_expression_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_expression_evaluation");

    let expressions = [
        "Base + Added",
        "Base * (1.0 + Increased)",
        "Base * (1.0 + Increased) + Added",
        "min(Base * (1.0 + Increased) + Added, Cap)",
        "(Base * (1.0 + Increased) + Added) * (1.0 + More) - Taken",
    ];

    for (i, expr_str) in expressions.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::from_parameter(i),
            expr_str,
            |b, &expr_src| {
                let (mut app, entity) = setup_app_with_entity();
                let expr_owned = expr_src.to_string();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(entity, "Base", 100.0);
                        stats.add_modifier(entity, "Added", 50.0);
                        stats.add_modifier(entity, "Increased", 0.3);
                        stats.add_modifier(entity, "More", 0.2);
                        stats.add_modifier(entity, "Taken", 25.0);
                        stats.add_modifier(entity, "Cap", 200.0);
                        let _ =
                            stats.add_expr_modifier(entity, "Result", &expr_owned);
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(entity, "Result"));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 8. Many modifiers on a single stat
// ---------------------------------------------------------------------------

pub fn bench_many_modifiers_on_stat(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_modifiers_on_stat");

    for modifier_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(modifier_count),
            &modifier_count,
            |b, &mc| {
                let (mut app, entity) = setup_app_with_entity();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        for _ in 0..mc {
                            stats.add_modifier(entity, "Power", 1.0);
                        }
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(entity, "Power"));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 9. Many distinct stats on one entity
// ---------------------------------------------------------------------------

pub fn bench_many_distinct_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("many_distinct_stats");

    for stat_count in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::from_parameter(stat_count),
            &stat_count,
            |b, &sc| {
                let (mut app, entity) = setup_app_with_entity();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        for i in 0..sc {
                            stats.add_modifier(
                                entity,
                                &format!("Stat{i}.value"),
                                i as f32,
                            );
                        }
                    })
                    .unwrap();
                app.update();

                let target = format!("Stat{}.value", sc / 2);
                b.iter(|| {
                    let t = target.clone();
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            black_box(stats.evaluate(entity, &t));
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 10. Expression compilation (bevy_attributes-specific)
// ---------------------------------------------------------------------------

pub fn bench_expression_compilation(c: &mut Criterion) {
    let _app = setup_app();
    let mut group = c.benchmark_group("expression_compilation");

    let expressions = [
        ("simple", "Base + Added"),
        ("medium", "Base * (1.0 + Increased) + Added"),
        ("complex", "(Base * (1.0 + Increased) + Added) * (1.0 + More) - Taken"),
        ("with_functions", "clamp(min(Base * (1.0 + Increased), Cap) + Added, 0.0, 9999.0)"),
    ];

    for (label, expr_str) in expressions {
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            &expr_str,
            |b, &src| {
                let interner = Interner::global();
                for name in ["Base", "Added", "Increased", "More", "Taken", "Cap"] {
                    interner.get_or_intern(name);
                }
                b.iter(|| {
                    black_box(Expr::compile(src, None).unwrap());
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// 11. Cached read vs forced re-evaluation
// ---------------------------------------------------------------------------

pub fn bench_cached_vs_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("cached_vs_evaluate");

    let (mut app, entity) = setup_app_with_entity();
    app.world_mut()
        .run_system_once(move |mut stats: AttributesMut| {
            stats.add_modifier(entity, "Base", 100.0);
            stats.add_modifier(entity, "Added", 50.0);
            let _ = stats.add_expr_modifier(entity, "Result", "Base + Added");
        })
        .unwrap();
    app.update();

    group.bench_function("cached_read", |b| {
        b.iter(|| {
            let attrs = app.world().get::<Attributes>(entity).unwrap();
            black_box(attrs.value("Result"));
        });
    });

    group.bench_function("forced_evaluate", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(move |mut stats: AttributesMut| {
                    black_box(stats.evaluate(entity, "Result"));
                })
                .unwrap();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 12. System-param & schedule overhead isolation
// ---------------------------------------------------------------------------

pub fn bench_system_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("system_overhead");

    let (mut app, entity) = setup_app_with_entity();
    app.world_mut()
        .run_system_once(move |mut stats: AttributesMut| {
            stats.add_modifier(entity, "Base", 100.0);
        })
        .unwrap();
    app.update();

    group.bench_function("run_system_once_empty", |b| {
        b.iter(|| {
            app.world_mut().run_system_once(|| {}).unwrap();
        });
    });

    group.bench_function("run_system_once_query", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(|_q: Query<&Attributes>| {})
                .unwrap();
        });
    });

    group.bench_function("run_system_once_attributes_mut", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(|_stats: AttributesMut| {})
                .unwrap();
        });
    });

    group.bench_function("app_update", |b| {
        b.iter(|| {
            app.update();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 13. Evaluate path comparison: by-name vs try_evaluate vs by-id
// ---------------------------------------------------------------------------

pub fn bench_evaluate_paths(c: &mut Criterion) {
    let mut group = c.benchmark_group("evaluate_paths");

    let (mut app, entity) = setup_app_with_entity();
    app.world_mut()
        .run_system_once(move |mut stats: AttributesMut| {
            stats.add_modifier(entity, "Base", 100.0);
            stats.add_modifier(entity, "Added", 50.0);
            let _ = stats.add_expr_modifier(entity, "Result", "Base + Added");
        })
        .unwrap();
    app.update();

    let result_id = Interner::global().get("Result").unwrap();

    group.bench_function("cached_read_by_name", |b| {
        b.iter(|| {
            let attrs = app.world().get::<Attributes>(entity).unwrap();
            black_box(attrs.value("Result"));
        });
    });

    group.bench_function("cached_read_by_id", |b| {
        b.iter(|| {
            let attrs = app.world().get::<Attributes>(entity).unwrap();
            black_box(attrs.get(result_id));
        });
    });

    group.bench_function("evaluate_by_name", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(move |mut stats: AttributesMut| {
                    black_box(stats.evaluate(entity, "Result"));
                })
                .unwrap();
        });
    });

    group.bench_function("try_evaluate", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(move |mut stats: AttributesMut| {
                    black_box(stats.try_evaluate(entity, "Result"));
                })
                .unwrap();
        });
    });

    group.bench_function("evaluate_by_id", |b| {
        b.iter(|| {
            app.world_mut()
                .run_system_once(move |mut stats: AttributesMut| {
                    black_box(stats.evaluate_id(entity, result_id));
                })
                .unwrap();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// 14. Propagation breakdown: isolate mutation, schedule, and read costs
// ---------------------------------------------------------------------------

pub fn bench_propagation_mutation(c: &mut Criterion) {
    let mut group = c.benchmark_group("propagation_mutation");

    for entity_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |b, &ec| {
                let (mut app, entities) = setup_app_with_entities(ec + 1);
                let central = entities[0];
                let deps: Vec<Entity> = entities[1..].to_vec();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(central, "Aura.base", 10.0);
                        for (i, &dep) in deps.iter().enumerate() {
                            stats.register_source(dep, "CentralSource", central);
                            let mult = 0.8 + ((i as f32 % 5.0) * 0.1);
                            let _ = stats.add_expr_modifier(
                                dep,
                                "Buff.value",
                                &format!("Aura.base@CentralSource * {mult}"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            stats.add_modifier(central, "Aura.base", 1.0);
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

pub fn bench_propagation_app_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("propagation_app_update");

    for entity_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |b, &ec| {
                let (mut app, entities) = setup_app_with_entities(ec + 1);
                let central = entities[0];
                let deps: Vec<Entity> = entities[1..].to_vec();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(central, "Aura.base", 10.0);
                        for (i, &dep) in deps.iter().enumerate() {
                            stats.register_source(dep, "CentralSource", central);
                            let mult = 0.8 + ((i as f32 % 5.0) * 0.1);
                            let _ = stats.add_expr_modifier(
                                dep,
                                "Buff.value",
                                &format!("Aura.base@CentralSource * {mult}"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    app.update();
                });
            },
        );
    }
    group.finish();
}

pub fn bench_propagation_read_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("propagation_read_cached");

    for entity_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |b, &ec| {
                let (mut app, entities) = setup_app_with_entities(ec + 1);
                let central = entities[0];
                let deps: Vec<Entity> = entities[1..].to_vec();
                let deps_setup = deps.clone();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(central, "Aura.base", 10.0);
                        for (i, &dep) in deps_setup.iter().enumerate() {
                            stats.register_source(dep, "CentralSource", central);
                            let mult = 0.8 + ((i as f32 % 5.0) * 0.1);
                            let _ = stats.add_expr_modifier(
                                dep,
                                "Buff.value",
                                &format!("Aura.base@CentralSource * {mult}"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                b.iter(|| {
                    for &dep in &deps {
                        let attrs = app.world().get::<Attributes>(dep).unwrap();
                        black_box(attrs.value("Buff.value"));
                    }
                });
            },
        );
    }
    group.finish();
}

pub fn bench_propagation_read_evaluate(c: &mut Criterion) {
    let mut group = c.benchmark_group("propagation_read_evaluate");

    for entity_count in [1, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::from_parameter(entity_count),
            &entity_count,
            |b, &ec| {
                let (mut app, entities) = setup_app_with_entities(ec + 1);
                let central = entities[0];
                let deps: Vec<Entity> = entities[1..].to_vec();
                let deps_setup = deps.clone();

                app.world_mut()
                    .run_system_once(move |mut stats: AttributesMut| {
                        stats.add_modifier(central, "Aura.base", 10.0);
                        for (i, &dep) in deps_setup.iter().enumerate() {
                            stats.register_source(dep, "CentralSource", central);
                            let mult = 0.8 + ((i as f32 % 5.0) * 0.1);
                            let _ = stats.add_expr_modifier(
                                dep,
                                "Buff.value",
                                &format!("Aura.base@CentralSource * {mult}"),
                            );
                        }
                    })
                    .unwrap();
                app.update();

                let buff_id = Interner::global().get("Buff.value").unwrap();

                b.iter(|| {
                    let deps_inner = deps.clone();
                    app.world_mut()
                        .run_system_once(move |mut stats: AttributesMut| {
                            for &dep in &deps_inner {
                                black_box(stats.evaluate_id(dep, buff_id));
                            }
                        })
                        .unwrap();
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_stat_access,
    bench_dependent_stats,
    bench_entity_dependencies,
    bench_tag_based_stats,
    bench_mixed_dependencies,
    bench_stats_update_propagation,
    bench_complex_expression_evaluation,
    bench_many_modifiers_on_stat,
    bench_many_distinct_stats,
    bench_expression_compilation,
    bench_cached_vs_evaluate,
    bench_system_overhead,
    bench_evaluate_paths,
    bench_propagation_mutation,
    bench_propagation_app_update,
    bench_propagation_read_cached,
    bench_propagation_read_evaluate,
);
criterion_main!(benches);
