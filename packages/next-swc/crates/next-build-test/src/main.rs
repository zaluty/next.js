#![feature(future_join)]
#![feature(min_specialization)]
#![feature(arbitrary_self_types)]

use std::{
    io::{stdout, Write},
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::Result;
use next_api::{
    project::{ProjectContainer, ProjectOptions},
    route::{Endpoint, Route},
};
use turbo_tasks::{TransientInstance, TurboTasks, TurboTasksApi, Vc};
use turbo_tasks_malloc::TurboMalloc;
use turbopack_binding::turbo::tasks_memory::MemoryBackend;

#[global_allocator]
static ALLOC: turbo_tasks_malloc::TurboMalloc = turbo_tasks_malloc::TurboMalloc;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .on_thread_stop(|| {
            TurboMalloc::thread_stop();
        })
        .build()
        .unwrap()
        .block_on(async {
            let tt = TurboTasks::new(MemoryBackend::new(6 * 1024 * 1024 * 1024));
            let r = main_inner(&tt).await;

            let start = Instant::now();
            drop(tt);
            println!("drop {:?}", start.elapsed());

            r
        })
        .unwrap();
}

async fn main_inner(tt: &TurboTasks<MemoryBackend>) -> Result<()> {
    register();

    let mut file = std::fs::File::open("project_options.json")?;
    let data: ProjectOptions = serde_json::from_reader(&mut file).unwrap();

    let options = ProjectOptions { ..data };

    let start = Instant::now();
    let project = tt
        .run_once(async { Ok(ProjectContainer::new(options)) })
        .await?;
    println!("ProjectContainer::new {:?} ({} GB)", start.elapsed(), mem());

    let start = Instant::now();
    let entrypoints = tt
        .run_once(async move { Ok(project.entrypoints().await?) })
        .await?;
    println!("project.entrypoints {:?} ({} GB)", start.elapsed(), mem());

    // TODO run 10 in parallel
    // select 100 by pseudo random
    // let selected_routes = [
    //     "/app-future/[lang]/home/[experiments]",
    //     "/api/feature-flags",
    //     "/api/show-consent-banner",
    //     "/api/jwt",
    //     "/api/exp",
    // ];
    let selected_routes = entrypoints.routes.keys().cloned().collect::<Vec<_>>();
    for name in selected_routes {
        let route = entrypoints.routes.get(&name).unwrap().clone();
        print!("{name}");
        stdout().flush().unwrap();
        let start = Instant::now();
        tt.run_once(async move {
            Ok(match route {
                Route::Page {
                    html_endpoint,
                    data_endpoint: _,
                } => {
                    html_endpoint.write_to_disk().await?;
                }
                Route::PageApi { endpoint } => {
                    endpoint.write_to_disk().await?;
                }
                Route::AppPage(routes) => {
                    for route in routes {
                        route.html_endpoint.write_to_disk().await?;
                    }
                }
                Route::AppRoute {
                    original_name: _,
                    endpoint,
                } => {
                    endpoint.write_to_disk().await?;
                }
                Route::Conflict => {
                    println!("WARN: conflict {}", name);
                }
            })
        })
        .await?;
        println!(" {:?} ({} GB)", start.elapsed(), mem());
        loop {
            let start = Instant::now();
            if tt.backend().run_gc(false, &*tt) {
                println!("GC {:?} ({} GB)...", start.elapsed(), mem());
            } else {
                println!("GC {:?} ({} GB) done", start.elapsed(), mem());
                break;
            }
        }
    }

    let session = TransientInstance::new(());
    let idents = tt
        .run_once(async move { Ok(project.hmr_identifiers().await?) })
        .await?;
    let start = Instant::now();
    let mut i = 0;
    for ident in idents {
        let session = session.clone();
        let start = Instant::now();
        let task = tt.spawn_root_task(move || {
            let session = session.clone();
            async move {
                let project = project.project();
                project
                    .hmr_update(
                        ident.clone(),
                        project.hmr_version_state(ident.clone(), session),
                    )
                    .await?;
                Ok(Vc::<()>::cell(()))
            }
        });
        tt.wait_task_completion(task, true).await?;
        let e = start.elapsed();
        if e.as_millis() > 10 {
            println!("HMR: {:?} {:?}", ident, e);
        }
        i += 1;
        if i > 20 {
            break;
        }
    }
    println!("HMR {:?} ({} GB)", start.elapsed(), mem());

    println!("Done ({}GB)", mem());

    loop {
        sleep(Duration::from_secs(1000));
    }

    Ok(())
}

fn mem() -> f32 {
    (TurboMalloc::memory_usage() / 1024 / 1024) as f32 / 1024.0
}

fn register() {
    next_api::register();
    include!(concat!(env!("OUT_DIR"), "/register.rs"));
}
