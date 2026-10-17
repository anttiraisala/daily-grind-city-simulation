//! End-to-end checks on the tiny town example city (50 residents, 10 cars, one main road,
//! side streets, a signalised junction and pedestrian crossings).

use citysim_core::map::load_map;
use citysim_core::scenario::Scenario;
use citysim_core::world::World;
use citysim_core::TICKS_PER_DAY;

fn town() -> World {
    let map = load_map(include_str!("../../../cities/tiny-town/map.json")).unwrap();
    let scenario =
        Scenario::from_json(include_str!("../../../cities/tiny-town/scenario.json")).unwrap();
    World::new(map, scenario).unwrap()
}

#[test]
fn the_town_loads_as_described() {
    let w = town();
    assert_eq!(w.resident_count(), 50);
    assert_eq!(w.car_owner_count(), 10);
    assert_eq!(w.map.crossings.len(), 8);
    assert_eq!(w.map.buildings.len(), 6);
    assert!(!w.map.derived_footpaths);
    assert!(w.warnings.is_empty(), "warnings: {:?}", w.warnings);
    // Four signalised crossings (two across Main Street, two across the side street).
    assert_eq!(
        w.map.crossings.iter().filter(|c| c.walk.is_some()).count(),
        4
    );
}

#[test]
fn everyone_completes_their_day_safely() {
    let mut w = town();
    let mut min_gap = f64::INFINITY;
    let mut peak_walkers = 0;
    let mut peak_cars = 0;
    for _ in 0..TICKS_PER_DAY {
        w.step();
        peak_walkers = peak_walkers.max(w.walker_count());
        peak_cars = peak_cars.max(w.traffic.vehicle_count());
        if let Some(g) = w.traffic.min_gap() {
            min_gap = min_gap.min(g);
        }
    }
    let s = &w.stats;
    eprintln!("{s:#?}\npeak walkers {peak_walkers}, peak cars {peak_cars}, min gap {min_gap:.2}");
    assert_eq!(
        s.trips_started, s.trips_done,
        "every trip that began must end"
    );
    assert_eq!(s.car_trips, 20, "10 commuters, two trips each");
    assert!(
        s.trips_done >= 90,
        "most residents travel: {}",
        s.trips_done
    );
    assert_eq!(w.walker_count(), 0);
    assert_eq!(w.traffic.vehicle_count(), 0);
    assert_eq!(
        s.crossing_violations, 0,
        "a vehicle drove over a pedestrian crossing in use"
    );
    assert_eq!(w.traffic.stats.clamps, 0);
    assert!(
        min_gap > 0.5,
        "cars must keep their distance (min gap {min_gap})"
    );
    assert!(s.crossings_made > 20, "pedestrians must use the crossings");
    assert!(
        s.crossing_wait_seconds > 0.0,
        "signals make pedestrians wait"
    );
    assert_eq!(s.unreachable, 0);
}

#[test]
fn workers_arrive_roughly_on_time() {
    let mut w = town();
    for _ in 0..TICKS_PER_DAY {
        w.step();
    }
    let workers = w.stats.commutes_car + w.stats.commutes_foot;
    assert!(workers >= 30);
    assert!(
        w.stats.late_arrivals * 4 <= workers,
        "{} of {workers} late",
        w.stats.late_arrivals
    );
}

#[test]
fn two_runs_are_identical() {
    let hash = || {
        let mut w = town();
        for _ in 0..200_000 {
            w.step();
        }
        w.state_hash()
    };
    assert_eq!(hash(), hash());
}

/// Everyone commutes by car at the same time, on top of the usual walkers.
#[test]
fn rush_hour_stress_keeps_crossings_safe() {
    let map = load_map(include_str!("../../../cities/tiny-town/map.json")).unwrap();
    let scenario = Scenario::from_json(
        r#"{"seed":3,"population":{"residents":50},
            "occupations":[
              {"kind":"office","share":0.66,"start":[{"time":"08:00","sdMinutes":0,"weight":1}],
               "durationHours":{"mean":1,"sd":0}},
              {"kind":"retired","share":0.34}],
            "commuteEstimate":{"bufferMinutes":{"mean":0,"sd":0}},
            "transport":{"forceCarCount":30}}"#,
    )
    .unwrap();
    let mut w = World::new(map, scenario).unwrap();
    let mut peak_cars = 0;
    for _ in 0..TICKS_PER_DAY {
        w.step();
        peak_cars = peak_cars.max(w.traffic.vehicle_count());
    }
    eprintln!("{:#?} peak cars {peak_cars}", w.stats);
    assert!(
        peak_cars >= 5,
        "cars must overlap in time (peak {peak_cars})"
    );
    assert_eq!(w.stats.crossing_violations, 0);
    assert_eq!(w.traffic.stats.clamps, 0);
    assert_eq!(w.stats.trips_started, w.stats.trips_done);
}
