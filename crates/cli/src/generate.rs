//! Generates the big example city: a square street grid with arterials, traffic lights on
//! arterial crossings, a business core, shops, schools and homes.

use std::fmt::Write;

pub struct BigCity {
    pub map_json: String,
    pub scenario_json: String,
    pub nodes: usize,
    pub edges: usize,
    pub buildings: usize,
    pub home_capacity: u32,
    pub jobs: u32,
    pub residents: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum Zone {
    Office,
    Shop,
    School,
    Hospital,
    Police,
    FireStation,
    Home,
}

fn zone(x: usize, y: usize, n: usize) -> Zone {
    let c = (n - 1) / 2;
    let d = x.abs_diff(c).max(y.abs_diff(c));
    let cells = n - 1;
    if d <= 4 && (x, y) != (c + 1, c) && (x, y) != (c + 2, c) && (x, y) != (c, c + 1) {
        return Zone::Office;
    }
    if (x, y) == (c + 1, c) {
        return Zone::Hospital;
    }
    if (x, y) == (c + 2, c) {
        return Zone::Police;
    }
    if (x, y) == (c, c + 1) {
        return Zone::FireStation;
    }
    if x >= cells || y >= cells {
        return Zone::Home;
    }
    if (x * 7 + y * 3).is_multiple_of(11) {
        return Zone::Shop;
    }
    if (x + 2 * y).is_multiple_of(23) {
        return Zone::School;
    }
    Zone::Home
}

pub fn big_city(n: usize, spacing: f64) -> BigCity {
    let id = |x: usize, y: usize| y * n + x + 1;
    let mut m = String::new();
    m.push_str("{\n  \"name\": \"big_city\",\n  \"defaults\": {\"laneWidthM\": 3.5, \"footpathWidthM\": 2.0},\n  \"nodes\": [\n");
    let mut first = true;
    let arterial = |i: usize| i.is_multiple_of(5);
    let light_id = |x: usize, y: usize| (y / 5) * (n.div_ceil(5)) + (x / 5) + 1;
    for y in 0..n {
        for x in 0..n {
            if !first {
                m.push_str(",\n");
            }
            first = false;
            let light = if arterial(x) && arterial(y) {
                format!(", \"trafficLightId\": {}", light_id(x, y))
            } else {
                String::new()
            };
            write!(
                m,
                "    {{\"id\": {}, \"pos\": {{\"x\": {}, \"y\": {}}}, \"level\": 0{light}}}",
                id(x, y),
                x as f64 * spacing,
                y as f64 * spacing
            )
            .unwrap();
        }
    }
    m.push_str("\n  ],\n  \"edges\": [\n");
    // Edge ids: horizontal edge from (x,y) to (x+1,y): 1 + y*(n-1) + x.
    // Vertical edge from (x,y) to (x,y+1): 1 + n*(n-1) + y*n + x.
    let h_edge = |x: usize, y: usize| 1 + y * (n - 1) + x;
    let v_edge = |x: usize, y: usize| 1 + n * (n - 1) + y * n + x;
    let mut edges = 0;
    let mut first = true;
    let mut push_edge = |m: &mut String, eid: usize, a: usize, b: usize, art: bool| {
        if !first {
            m.push_str(",\n");
        }
        first = false;
        write!(
            m,
            "    {{\"id\": {eid}, \"from\": {a}, \"to\": {b}, \"kind\": \"road\", \"lanes\": {}, \"speedLimit\": {}, \"oneWay\": false}}",
            if art { 2 } else { 1 },
            if art { 13.9 } else { 8.3 }
        )
        .unwrap();
        edges += 1;
    };
    for y in 0..n {
        for x in 0..n - 1 {
            push_edge(&mut m, h_edge(x, y), id(x, y), id(x + 1, y), arterial(y));
        }
    }
    for y in 0..n - 1 {
        for x in 0..n {
            push_edge(&mut m, v_edge(x, y), id(x, y), id(x, y + 1), arterial(x));
        }
    }
    m.push_str("\n  ],\n  \"trafficLights\": [\n");
    let mut first = true;
    let mut lights = 0;
    for y in (0..n).step_by(5) {
        for x in (0..n).step_by(5) {
            if !first {
                m.push_str(",\n");
            }
            first = false;
            lights += 1;
            let mut h = Vec::new();
            let mut v = Vec::new();
            if x > 0 {
                h.push(h_edge(x - 1, y));
            }
            if x + 1 < n {
                h.push(h_edge(x, y));
            }
            if y > 0 {
                v.push(v_edge(x, y - 1));
            }
            if y + 1 < n {
                v.push(v_edge(x, y));
            }
            let offset = (x / 5 * 11 + y / 5 * 7) % 60;
            let list = |l: &Vec<usize>| {
                l.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            write!(
                m,
                "    {{\"id\": {}, \"nodeId\": {}, \"cycleSeconds\": 60, \"phases\": [{{\"edgeIds\": [{}], \"startSecond\": {offset}, \"greenSeconds\": 26}}, {{\"edgeIds\": [{}], \"startSecond\": {}, \"greenSeconds\": 26}}]}}",
                light_id(x, y),
                id(x, y),
                list(&h),
                list(&v),
                (offset + 30) % 60
            )
            .unwrap();
        }
    }
    m.push_str("\n  ],\n  \"buildings\": [\n");
    let inset = spacing * 0.2;
    let mut first = true;
    let (mut home_capacity, mut jobs, mut count) = (0u32, 0u32, 0usize);
    for y in 0..n - 1 {
        for x in 0..n - 1 {
            let z = zone(x, y, n);
            let (kind, cap, open) = match z {
                Zone::Office => (
                    "office",
                    "\"jobs\": 600, \"visitors\": 100",
                    "360, \"to\": 1140",
                ),
                Zone::Shop => (
                    "shop",
                    "\"jobs\": 25, \"visitors\": 120",
                    "480, \"to\": 1260",
                ),
                Zone::School => (
                    "school",
                    "\"jobs\": 150, \"visitors\": 0",
                    "420, \"to\": 960",
                ),
                Zone::Hospital => (
                    "hospital",
                    "\"jobs\": 1500, \"visitors\": 300",
                    "0, \"to\": 1440",
                ),
                Zone::Police => (
                    "police",
                    "\"jobs\": 200, \"visitors\": 20",
                    "0, \"to\": 1440",
                ),
                Zone::FireStation => (
                    "firestation",
                    "\"jobs\": 100, \"visitors\": 0",
                    "0, \"to\": 1440",
                ),
                Zone::Home => ("home", "\"residents\": 60", "0, \"to\": 1440"),
            };
            match z {
                Zone::Home => home_capacity += 60,
                Zone::Office => jobs += 600,
                Zone::Shop => jobs += 25,
                Zone::School => jobs += 150,
                Zone::Hospital => jobs += 1500,
                Zone::Police => jobs += 200,
                Zone::FireStation => jobs += 100,
            }
            if !first {
                m.push_str(",\n");
            }
            first = false;
            count += 1;
            let (x0, y0) = (x as f64 * spacing + inset, y as f64 * spacing + inset);
            let (x1, y1) = (
                (x + 1) as f64 * spacing - inset,
                (y + 1) as f64 * spacing - inset,
            );
            write!(
                m,
                "    {{\"id\": {count}, \"kind\": \"{kind}\", \"footprint\": [{{\"x\": {x0}, \"y\": {y0}}}, {{\"x\": {x1}, \"y\": {y0}}}, {{\"x\": {x1}, \"y\": {y1}}}, {{\"x\": {x0}, \"y\": {y1}}}], \"entranceNodeId\": {}, \"capacity\": {{{cap}}}, \"open\": [{{\"from\": {open}}}], \"flammability\": 0.4}}",
                id(x, y)
            )
            .unwrap();
        }
    }
    m.push_str("\n  ],\n  \"mobileWorkplaces\": []\n}\n");
    let _ = lights;

    let residents = (home_capacity as f64 * 0.95) as u32;
    let scenario = format!(
        r#"{{
  "seed": 1,
  "time": {{ "tickSeconds": 0.1, "startTime": "00:00", "daysToRun": 1 }},
  "population": {{ "residents": {residents} }},
  "occupations": [
    {{
      "kind": "office", "share": 0.38, "remoteShare": 0.15,
      "start": [
        {{ "time": "08:00", "sdMinutes": 30, "weight": 0.5 }},
        {{ "time": "07:00", "sdMinutes": 20, "weight": 0.2 }},
        {{ "time": "10:00", "sdMinutes": 30, "weight": 0.3 }}
      ],
      "durationHours": {{ "mean": 8, "sd": 0.7 }}
    }},
    {{
      "kind": "parttime", "share": 0.10, "remoteShare": 0.10,
      "start": [{{ "time": "09:00", "sdMinutes": 90, "weight": 1 }}],
      "durationHours": {{ "mean": 5, "sd": 1 }}
    }},
    {{ "kind": "shift", "share": 0.12, "shifts": ["06:00-14:00", "14:00-22:00", "22:00-06:00"] }},
    {{
      "kind": "student", "share": 0.12,
      "start": [{{ "time": "08:30", "sdMinutes": 15, "weight": 1 }}],
      "durationHours": {{ "mean": 6, "sd": 1 }}
    }},
    {{ "kind": "retired", "share": 0.14 }},
    {{ "kind": "unemployed", "share": 0.14 }}
  ],
  "commuteEstimate": {{ "initialFactor": 1.2, "learningRate": 0.3, "bufferMinutes": {{ "mean": 5, "sd": 3 }} }},
  "transport": {{
    "ownership": {{ "car": 0.55, "bike": 0.35 }},
    "forceCarCount": null,
    "vehicleSizes": {{ "car": {{ "lengthM": 4.5, "widthM": 1.8 }} }}
  }}
}}
"#
    );
    BigCity {
        map_json: m,
        scenario_json: scenario,
        nodes: n * n,
        edges,
        buildings: count,
        home_capacity,
        jobs,
        residents,
    }
}
