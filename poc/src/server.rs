//! Minimal HTTP server (std only): serves the viewer page and a small JSON/binary API.
//!
//! The simulation runs on its own thread and is the single source of truth; request
//! handlers only read snapshots or queue commands. The production version is meant
//! to use a WebSocket and a proper HTTP library instead of polling.

use crate::sim::{hhmm, Command, Sim, View, DAY};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

const INDEX_HTML: &str = include_str!("../web/index.html");

struct Shared {
    sim: Sim,
    paused: bool,
    /// Requested simulated seconds per wall-clock second.
    speed: f64,
    /// Measured simulated seconds per wall-clock second.
    actual_speed: f64,
    budget: f64,
}

pub struct Options {
    pub bind: String,
    pub speed: f64,
    pub token: String,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|e| e.into_inner())
}

/// Address of this machine on the local network (no traffic is sent).
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

pub fn serve(sim: Sim, opts: Options) -> std::io::Result<()> {
    let listener = TcpListener::bind(&opts.bind)?;
    let port = listener.local_addr()?.port();
    let shared = Arc::new(Mutex::new(Shared {
        sim,
        paused: false,
        speed: opts.speed,
        actual_speed: 0.0,
        budget: 0.0,
    }));
    {
        let shared = shared.clone();
        thread::spawn(move || sim_loop(&shared));
    }

    println!(
        "Viewer (this computer): http://localhost:{port}/?t={}",
        opts.token
    );
    if opts.bind.starts_with("0.0.0.0") {
        match lan_ip() {
            Some(ip) => println!(
                "Viewer (same network, e.g. phone): http://{ip}:{port}/?t={}",
                opts.token
            ),
            None => println!(
                "Viewer (same network): http://<this-computer-ip>:{port}/?t={}",
                opts.token
            ),
        }
    }
    println!("The token in the URL is required for all API calls. Press Ctrl+C to stop.");

    let token = Arc::new(opts.token);
    for stream in listener.incoming().flatten() {
        let shared = shared.clone();
        let token = token.clone();
        thread::spawn(move || {
            let _ = handle(stream, &shared, &token);
        });
    }
    Ok(())
}

/// Advances the simulation in real time, scaled by the requested speed.
fn sim_loop(shared: &Mutex<Shared>) {
    let mut last = Instant::now();
    let mut window_start = Instant::now();
    let mut window_ticks = 0u64;
    loop {
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.25);
        last = now;
        {
            let mut g = lock(shared);
            if !g.paused {
                g.budget += g.speed * dt;
                let wanted = g.budget.floor() as u64;
                let deadline = Instant::now() + Duration::from_millis(40);
                let mut done = 0u64;
                while done < wanted {
                    g.sim.step();
                    done += 1;
                    if done % 16 == 0 && Instant::now() > deadline {
                        break;
                    }
                }
                g.budget -= done as f64;
                if done < wanted {
                    g.budget = 0.0; // cannot keep up: drop the backlog instead of spiralling
                }
                window_ticks += done;
            }
            let el = window_start.elapsed().as_secs_f64();
            if el >= 0.5 {
                g.actual_speed = window_ticks as f64 / el;
                window_ticks = 0;
                window_start = Instant::now();
            }
        }
        thread::sleep(Duration::from_millis(4));
    }
}

fn respond(stream: &mut TcpStream, code: u16, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\
Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\n\
Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn json(stream: &mut TcpStream, code: u16, body: String) -> std::io::Result<()> {
    respond(stream, code, "application/json", body.as_bytes())
}

fn num<T: std::str::FromStr>(q: &HashMap<&str, &str>, key: &str) -> Option<T> {
    q.get(key).and_then(|v| v.parse().ok())
}

fn handle(mut stream: TcpStream, shared: &Mutex<Shared>, token: &str) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 16_384 {
            break;
        }
    }
    let text = String::from_utf8_lossy(&buf);
    let first = text.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let q: HashMap<&str, &str> = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .collect();

    if path == "/" && method == "GET" {
        return respond(
            &mut stream,
            200,
            "text/html; charset=utf-8",
            INDEX_HTML.as_bytes(),
        );
    }
    let Some(api) = path.strip_prefix("/api/") else {
        return json(&mut stream, 404, "{\"error\":\"not found\"}".into());
    };
    if q.get("t").copied() != Some(token) {
        return json(
            &mut stream,
            403,
            "{\"error\":\"missing or wrong token\"}".into(),
        );
    }

    match (method, api) {
        ("GET", "info") => {
            let g = lock(shared);
            let body = format!(
                "{{\"nodes_w\":{},\"nodes_h\":{},\"block_m\":{},\"agents\":{},\"seed\":{},\"day_seconds\":{}}}",
                g.sim.w,
                g.sim.h,
                crate::sim::BLOCK_M,
                g.sim.agent_count(),
                g.sim.cfg.seed,
                DAY
            );
            json(&mut stream, 200, body)
        }
        ("GET", "status") => {
            let g = lock(shared);
            let s = &g.sim.stats;
            let body = format!(
                "{{\"time\":{},\"day\":{},\"clock\":\"{}\",\"paused\":{},\"speed\":{},\"actual_speed\":{:.1},\
\"moving\":{},\"cars\":{},\"avg_speed_pct\":{:.1},\"late_today\":{},\"trips_done\":{},\"commands\":{}}}",
                g.sim.time,
                g.sim.time / DAY + 1,
                hhmm(g.sim.time),
                g.paused,
                g.speed,
                g.actual_speed,
                g.sim.moving_count(),
                s.cars_moving,
                s.avg_speed_permille as f64 / 10.0,
                s.late_today,
                s.trips_done,
                g.sim.command_log.len()
            );
            json(&mut stream, 200, body)
        }
        ("GET", "snapshot") => {
            let view = View {
                x0: num(&q, "x0").unwrap_or(-1e9),
                y0: num(&q, "y0").unwrap_or(-1e9),
                x1: num(&q, "x1").unwrap_or(1e9),
                y1: num(&q, "y1").unwrap_or(1e9),
            };
            let cap = num::<usize>(&q, "cap")
                .unwrap_or(60_000)
                .clamp(100, 250_000);
            let body = lock(shared).sim.snapshot(&view, cap);
            respond(&mut stream, 200, "application/octet-stream", &body)
        }
        ("GET", "heat") => {
            let body = lock(shared).sim.heat_bytes();
            respond(&mut stream, 200, "application/octet-stream", &body)
        }
        ("GET", "agent") => {
            match num::<usize>(&q, "id").and_then(|id| lock(shared).sim.agent_json(id)) {
                Some(body) => json(&mut stream, 200, body),
                None => json(&mut stream, 404, "{\"error\":\"no such agent\"}".into()),
            }
        }
        ("GET", "commands") => {
            let g = lock(shared);
            let mut out = String::new();
            for (tick, c) in &g.sim.command_log {
                out.push_str(&format!("tick {tick}\t{c:?}\n"));
            }
            respond(
                &mut stream,
                200,
                "text/plain; charset=utf-8",
                out.as_bytes(),
            )
        }
        ("POST", "cmd") => {
            let mut g = lock(shared);
            let result = match q.get("kind").copied() {
                Some("pause") => {
                    g.paused = true;
                    Ok(())
                }
                Some("resume") => {
                    g.paused = false;
                    Ok(())
                }
                Some("speed") => match num::<f64>(&q, "value") {
                    Some(v) if (0.0..=1_000_000.0).contains(&v) => {
                        g.speed = v;
                        Ok(())
                    }
                    _ => Err("speed must be a number between 0 and 1000000"),
                },
                Some("delay") => match (num::<u32>(&q, "agent"), num::<u32>(&q, "seconds")) {
                    (Some(agent), Some(seconds)) => {
                        g.sim.submit(Command::Delay { agent, seconds });
                        Ok(())
                    }
                    _ => Err("delay needs agent and seconds"),
                },
                _ => Err("unknown command"),
            };
            drop(g);
            match result {
                Ok(()) => json(&mut stream, 200, "{\"ok\":true}".into()),
                Err(e) => json(
                    &mut stream,
                    400,
                    format!("{{\"ok\":false,\"error\":\"{e}\"}}"),
                ),
            }
        }
        (_, "cmd") => json(&mut stream, 405, "{\"error\":\"use POST\"}".into()),
        _ => json(&mut stream, 404, "{\"error\":\"not found\"}".into()),
    }
}
