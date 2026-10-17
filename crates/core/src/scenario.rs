//! Reads the parts of a scenario file that the core uses so far. Everything else in the
//! file (needs, events, emergency, output, trait distributions ...) is accepted and ignored
//! until the matching feature exists.

use crate::json::{self, Value};

#[derive(Clone, Debug)]
pub struct StartTime {
    pub minute: f64,
    pub sd_minutes: f64,
    pub weight: f64,
}

#[derive(Clone, Debug)]
pub struct Occupation {
    pub kind: String,
    pub share: f64,
    pub remote_share: f64,
    pub starts: Vec<StartTime>,
    /// (mean, sd) hours at work.
    pub duration_hours: (f64, f64),
    /// Shifts as (start minute, duration hours); used instead of `starts` if present.
    pub shifts: Vec<(f64, f64)>,
}

impl Occupation {
    /// Does this occupation have a workplace to go to?
    pub fn has_workplace(&self) -> bool {
        !self.starts.is_empty() || !self.shifts.is_empty()
    }
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub seed: u64,
    pub residents: u32,
    pub days: u32,
    /// Exact number of car owners among commuters, overriding `car_share`.
    pub force_car_count: Option<u32>,
    pub car_share: f64,
    pub bike_share: f64,
    pub occupations: Vec<Occupation>,
    pub commute_initial_factor: f64,
    pub commute_learning_rate: f64,
    pub buffer_minutes: (f64, f64),
}

impl Default for Scenario {
    fn default() -> Self {
        Scenario {
            seed: 1,
            residents: 100,
            days: 1,
            force_car_count: None,
            car_share: 0.55,
            bike_share: 0.0,
            occupations: vec![
                Occupation {
                    kind: "office".into(),
                    share: 0.7,
                    remote_share: 0.0,
                    starts: vec![StartTime {
                        minute: 480.0,
                        sd_minutes: 30.0,
                        weight: 1.0,
                    }],
                    duration_hours: (8.0, 0.7),
                    shifts: Vec::new(),
                },
                Occupation {
                    kind: "retired".into(),
                    share: 0.3,
                    remote_share: 0.0,
                    starts: Vec::new(),
                    duration_hours: (0.0, 0.0),
                    shifts: Vec::new(),
                },
            ],
            commute_initial_factor: 1.2,
            commute_learning_rate: 0.3,
            buffer_minutes: (5.0, 3.0),
        }
    }
}

/// "08:30" to minutes after midnight.
pub fn parse_clock(s: &str) -> Result<f64, String> {
    let (h, m) = s
        .split_once(':')
        .ok_or_else(|| format!("bad time \"{s}\" (expected HH:MM)"))?;
    let h: f64 = h.trim().parse().map_err(|_| format!("bad time \"{s}\""))?;
    let m: f64 = m.trim().parse().map_err(|_| format!("bad time \"{s}\""))?;
    if !(0.0..=24.0).contains(&h) || !(0.0..60.0).contains(&m) {
        return Err(format!("bad time \"{s}\""));
    }
    Ok(h * 60.0 + m)
}

fn mean_sd(v: Option<&Value>, default: (f64, f64)) -> (f64, f64) {
    match v {
        Some(v) => (
            v.get("mean").and_then(Value::as_f64).unwrap_or(default.0),
            v.get("sd").and_then(Value::as_f64).unwrap_or(default.1),
        ),
        None => default,
    }
}

impl Scenario {
    pub fn from_json(text: &str) -> Result<Scenario, String> {
        let root = json::parse(text)?;
        let mut s = Scenario::default();
        if let Some(v) = root.get("seed").and_then(Value::as_f64) {
            s.seed = v as u64;
        }
        if let Some(v) = root
            .at(&["population", "residents"])
            .and_then(Value::as_f64)
        {
            if v < 0.0 || v.fract() != 0.0 {
                return Err("population.residents must be a whole number".into());
            }
            s.residents = v as u32;
        }
        if let Some(v) = root.at(&["time", "daysToRun"]).and_then(Value::as_f64) {
            s.days = (v as u32).max(1);
        }
        if let Some(v) = root
            .at(&["transport", "forceCarCount"])
            .and_then(Value::as_f64)
        {
            s.force_car_count = Some(v as u32);
        }
        if let Some(v) = root
            .at(&["transport", "ownership", "car"])
            .and_then(Value::as_f64)
        {
            s.car_share = v.clamp(0.0, 1.0);
        }
        s.bike_share = root
            .at(&["transport", "ownership", "bike"])
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
            .clamp(0.0, 1.0);
        if let Some(v) = root
            .at(&["commuteEstimate", "initialFactor"])
            .and_then(Value::as_f64)
        {
            s.commute_initial_factor = v.max(0.1);
        }
        if let Some(v) = root
            .at(&["commuteEstimate", "learningRate"])
            .and_then(Value::as_f64)
        {
            s.commute_learning_rate = v.clamp(0.0, 1.0);
        }
        s.buffer_minutes = mean_sd(
            root.at(&["commuteEstimate", "bufferMinutes"]),
            s.buffer_minutes,
        );

        if let Some(list) = root.get("occupations").and_then(Value::as_array) {
            let mut occ = Vec::new();
            for o in list {
                let kind = o
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or("occupation without \"kind\"")?
                    .to_string();
                let mut starts = Vec::new();
                for st in o.get("start").and_then(Value::as_array).unwrap_or(&[]) {
                    let t = st
                        .get("time")
                        .and_then(Value::as_str)
                        .ok_or("start without \"time\"")?;
                    starts.push(StartTime {
                        minute: parse_clock(t)?,
                        sd_minutes: st.get("sdMinutes").and_then(Value::as_f64).unwrap_or(0.0),
                        weight: st.get("weight").and_then(Value::as_f64).unwrap_or(1.0),
                    });
                }
                let mut shifts = Vec::new();
                for sh in o.get("shifts").and_then(Value::as_array).unwrap_or(&[]) {
                    let text = sh.as_str().ok_or("a shift must be text like 06:00-14:00")?;
                    let (a, b) = text
                        .split_once('-')
                        .ok_or_else(|| format!("bad shift \"{text}\""))?;
                    let (a, b) = (parse_clock(a)?, parse_clock(b)?);
                    let dur = (b - a).rem_euclid(1440.0) / 60.0;
                    shifts.push((a, if dur == 0.0 { 24.0 } else { dur }));
                }
                occ.push(Occupation {
                    kind,
                    share: o
                        .get("share")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0)
                        .max(0.0),
                    remote_share: o
                        .get("remoteShare")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0),
                    starts,
                    duration_hours: mean_sd(o.get("durationHours"), (8.0, 0.5)),
                    shifts,
                });
            }
            if occ.is_empty() || occ.iter().map(|o| o.share).sum::<f64>() <= 0.0 {
                return Err("occupations need at least one positive share".into());
            }
            s.occupations = occ;
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_example_scenario_loads() {
        let s = Scenario::from_json(include_str!("../../../schema/scenario.example.json")).unwrap();
        assert_eq!(s.seed, 12345);
        assert_eq!(s.residents, 2000);
        assert_eq!(s.days, 7);
        assert_eq!(s.occupations.len(), 6);
        let office = &s.occupations[0];
        assert_eq!(office.starts.len(), 3);
        assert_eq!(office.starts[1].minute, 420.0);
        let shift = &s.occupations[2];
        assert_eq!(
            shift.shifts,
            vec![(360.0, 8.0), (840.0, 8.0), (1320.0, 8.0)]
        );
        assert!(s.force_car_count.is_none());
        assert!((s.car_share - 0.55).abs() < 1e-9);
    }

    #[test]
    fn bad_values_are_rejected() {
        assert!(Scenario::from_json(r#"{"population":{"residents":-4}}"#).is_err());
        assert!(Scenario::from_json(r#"{"occupations":[{"kind":"office","share":0}]}"#).is_err());
        assert!(parse_clock("25:00").is_err());
        assert!(parse_clock("8").is_err());
    }
}
