//! One-shot presentation-only route query. No IO, ECS mutation or wire output.

use std::collections::HashSet;
use mir2_client_bevy::{quest_route_search::{search_region, SearchError}, quest_supplies::SupplyVendor, quest_ui::QuestRouteTarget};
use serde::{Deserialize, Serialize};

const MAX_QUERY_BYTES: usize = 256 * 1024;
const MAX_CELLS: usize = 16_777_216;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
struct Point { x: i32, y: i32 }
impl Point { fn pair(self) -> (i32,i32) { (self.x,self.y) } }

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Stamp {
    generation: u64, revision: u64, connection_generation: u64,
    session_generation: u64, scene_revision: u64, player_object_id: u32, map_file_name: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(tag="type", rename_all="camelCase", deny_unknown_fields)]
enum Target {
    Entrance,
    HuntRegion { #[serde(rename="monsterIndex")] monster_index: i32, radius: i32 },
    Supply { vendor: Vendor },
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all="camelCase")]
enum Vendor { Potions, General, Poison }
impl Vendor {
    fn shared(self) -> SupplyVendor {
        match self { Self::Potions=>SupplyVendor::Potions, Self::General=>SupplyVendor::General, Self::Poison=>SupplyVendor::Poison }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Edge { from: Point, to: Point }

#[derive(Debug, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Query {
    version: u32, map_index: i32, map_file_name: String, width: u16, height: u16,
    origin: Point, destination: Point, route_target: Target,
    occupied_tiles: Vec<Point>, rejected_edges: Vec<Edge>, stamp: Stamp,
}

fn canonical_map(file: &str) -> bool {
    !file.is_empty() && file.len()<=128 && !file.contains("..") && !file.ends_with(".map")
        && file.as_bytes()[0].is_ascii_alphanumeric()
        && file.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c,b'_'|b'-'|b'.'))
}

impl Query {
    fn inside(&self, p: Point) -> bool {
        p.x>=0 && p.y>=0 && p.x<i32::from(self.width) && p.y<i32::from(self.height)
    }
    fn valid(&self, bits: &[u8]) -> bool {
        let Some(cells)=usize::from(self.width).checked_mul(usize::from(self.height)) else { return false; };
        let safe=|n| (1..=MAX_SAFE_JS_INTEGER).contains(&n);
        self.version==1 && self.map_index>=0 && canonical_map(&self.map_file_name)
            && self.width>0 && self.height>0 && cells<=MAX_CELLS
            && bits.len()==cells.div_ceil(8)
            && (cells%8==0 || bits.last().is_some_and(|last| *last >> (cells%8)==0))
            && safe(self.stamp.generation) && safe(self.stamp.revision)
            && safe(self.stamp.connection_generation) && safe(self.stamp.session_generation)
            && safe(self.stamp.scene_revision) && self.stamp.player_object_id>0
            && self.stamp.map_file_name==self.map_file_name
            && self.inside(self.origin) && self.inside(self.destination)
            && self.occupied_tiles.len()<=4096 && self.occupied_tiles.iter().all(|p| self.inside(*p))
            && self.rejected_edges.len()<=512 && self.rejected_edges.iter().all(|e|
                self.inside(e.from) && self.inside(e.to) && e.from!=e.to
                    && e.from.x.abs_diff(e.to.x)<=1 && e.from.y.abs_diff(e.to.y)<=1)
    }
    fn radius(&self) -> Result<i32,&'static str> {
        let target=match self.route_target {
            Target::Entrance=>QuestRouteTarget::Entrance,
            Target::HuntRegion {monster_index,radius}=> {
                if monster_index<0 || radius<0 || radius>i32::from(self.width.max(self.height)) { return Err("invalidTarget"); }
                QuestRouteTarget::HuntRegion {monster_index,radius}
            },
            Target::Supply {vendor}=>QuestRouteTarget::Supply {vendor:vendor.shared()},
        };
        match target {
            QuestRouteTarget::Entrance=>Ok(0),
            QuestRouteTarget::HuntRegion {radius,..}=>Ok(radius),
            QuestRouteTarget::Supply {vendor}=> {
                let route=vendor.route(self.map_index).ok_or("invalidDestination")?;
                if (route.x,route.y)!=self.destination.pair() { return Err("invalidDestination"); }
                Ok(if route.is_entrance {0} else {2})
            },
        }
    }
}

fn response(query: &Query, radius: Option<i32>, result: Result<Vec<(i32,i32)>,&str>) -> String {
    let (ok,steps,destination,error)=match result {
        Ok(steps)=> {
            let (x,y)=steps.last().copied().unwrap_or(query.origin.pair());
            (true,steps.into_iter().map(|(x,y)| Point{x,y}).collect::<Vec<_>>(),Some(Point{x,y}),None)
        },
        Err(error)=>(false,Vec::new(),None,Some(error)),
    };
    serde_json::json!({"version":1,"ok":ok,"mapIndex":query.map_index,"mapFileName":query.map_file_name,
        "origin":query.origin,"steps":steps,"destination":destination,"radius":radius,"error":error,"stamp":query.stamp}).to_string()
}

pub(crate) fn plan(query_json: &str, bits: &[u8]) -> String {
    let invalid=|| serde_json::json!({"version":1,"ok":false,"error":"invalidInput"}).to_string();
    if query_json.len()>MAX_QUERY_BYTES || bits.len()>MAX_CELLS.div_ceil(8) { return invalid(); }
    let Ok(query)=serde_json::from_str::<Query>(query_json) else { return invalid(); };
    if !query.valid(bits) { return invalid(); }
    let radius=match query.radius() { Ok(radius)=>radius, Err(error)=>return response(&query,None,Err(error)) };
    let occupied=query.occupied_tiles.iter().map(|p|p.pair()).collect::<HashSet<_>>();
    let rejected=query.rejected_edges.iter().map(|e|(e.from.pair(),e.to.pair())).collect::<HashSet<_>>();
    let static_blocked=|p:(i32,i32)| {
        let index=p.0 as usize * usize::from(query.height) + p.1 as usize;
        bits[index/8] & (1<<(index%8)) != 0
    };
    if radius==0 {
        if static_blocked(query.destination.pair()) { return response(&query,Some(radius),Err("destinationBlocked")); }
        if occupied.contains(&query.destination.pair()) { return response(&query,Some(radius),Err("destinationOccupied")); }
    } else if query.origin.x.abs_diff(query.destination.x).max(query.origin.y.abs_diff(query.destination.y))<=radius as u32
        && (static_blocked(query.origin.pair()) || occupied.contains(&query.origin.pair())) {
        return response(&query,Some(radius),Err("originBlocked"));
    }
    let result=search_region(i32::from(query.width),i32::from(query.height),query.origin.pair(),query.destination.pair(),radius,
        |from,to| static_blocked(to) || occupied.contains(&to) || rejected.contains(&(from,to)))
        .map_err(|error| match error {SearchError::OutsideMap=>"outsideMap",SearchError::BudgetExceeded=>"searchBudgetExceeded",SearchError::Unreachable=>"unreachable"});
    response(&query,Some(radius),result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json,Value};
    fn query() -> Value {
        json!({"version":1,"mapIndex":0,"mapFileName":"0","width":80,"height":80,
            "origin":{"x":10,"y":30},"destination":{"x":50,"y":30},
            "routeTarget":{"type":"huntRegion","monsterIndex":1,"radius":4},
            "occupiedTiles":[],"rejectedEdges":[],"stamp":{"generation":7,"revision":9,
                "connectionGeneration":2,"sessionGeneration":3,"sceneRevision":4,"playerObjectId":1000,"mapFileName":"0"}})
    }
    fn run(q:&Value,bits:&[u8]) -> Value { serde_json::from_str(&plan(&q.to_string(),bits)).unwrap() }
    #[test]
    fn region_uses_full_target_and_echoes_stamp_without_mutating_it() {
        let mut q=query();q["occupiedTiles"]=json!([{"x":46,"y":30},{"x":50,"y":30}]);
        let r=run(&q,&vec![0;800]);assert_eq!(r["ok"],true);assert_eq!(r["steps"].as_array().unwrap().len(),36);
        assert_ne!(r["destination"],json!({"x":46,"y":30}));assert_eq!(r["stamp"],q["stamp"]);
    }
    #[test]
    fn strict_query_rejects_extra_fields_bad_stamp_bitmap_padding_and_oversized_map() {
        let q=query();assert_eq!(run(&q,&vec![0;799])["error"],"invalidInput");
        let mut q=query();q["unexpected"]=json!(0);assert_eq!(run(&q,&vec![0;800])["error"],"invalidInput");
        let mut q=query();q["stamp"]["mapFileName"]=json!("1");assert_eq!(run(&q,&vec![0;800])["error"],"invalidInput");
        let mut q=query();q["width"]=json!(81);q["height"]=json!(81);let mut bits=vec![0;821];bits[820]=2;
        assert_eq!(run(&q,&bits)["error"],"invalidInput");
        let mut q=query();q["width"]=json!(65535);q["height"]=json!(65535);assert_eq!(run(&q,&[])["error"],"invalidInput");
    }
    #[test]
    fn blocked_current_area_and_occupied_exact_entrance_are_not_arrivals() {
        let mut q=query();q["origin"]=q["destination"].clone();q["occupiedTiles"]=json!([q["origin"].clone()]);
        assert_eq!(run(&q,&vec![0;800])["error"],"originBlocked");q["routeTarget"]=json!({"type":"entrance"});
        assert_eq!(run(&q,&vec![0;800])["error"],"destinationOccupied");
        q["routeTarget"]=json!({"type":"huntRegion","monsterIndex":1,"radius":4});q["occupiedTiles"]=json!([]);
        let r=run(&q,&vec![0;800]);assert_eq!(r["ok"],true);assert_eq!(r["steps"],json!([]));
        let mut bits=vec![0;800];let index=50*80+30;bits[index/8]|=1<<(index%8);
        assert_eq!(run(&q,&bits)["error"],"originBlocked");
    }
    #[test]
    fn x_major_bitmap_and_directed_edge_preserve_real_collision() {
        let mut q=query();q["routeTarget"]=json!({"type":"entrance"});let mut bits=vec![0;800];let index=50*80+30;bits[index/8]|=1<<(index%8);
        assert_eq!(run(&q,&bits)["error"],"destinationBlocked");
        q["rejectedEdges"]=json!([{"from":{"x":10,"y":30},"to":{"x":11,"y":30}}]);
        let r=run(&q,&vec![0;800]);assert_eq!(r["ok"],true);assert_ne!(r["steps"][0],json!({"x":11,"y":30}));
    }

    #[test]
    fn supply_goal_uses_shared_vendor_radius_and_rejects_changed_destination() {
        let destination=SupplyVendor::Potions.destination().expect("bundled supply destination");
        let width=(destination.x+10).max(80) as u16;
        let height=(destination.y+10).max(80) as u16;
        let mut q=query();q["mapIndex"]=json!(destination.map_index);q["width"]=json!(width);q["height"]=json!(height);
        q["origin"]=json!({"x":destination.x-5,"y":destination.y});
        q["destination"]=json!({"x":destination.x,"y":destination.y});q["routeTarget"]=json!({"type":"supply","vendor":"potions"});
        q["occupiedTiles"]=json!([q["destination"].clone()]);
        let bits=vec![0;(usize::from(width)*usize::from(height)).div_ceil(8)];
        let r=run(&q,&bits);assert_eq!(r["ok"],true);assert_eq!(r["radius"],2);
        assert_ne!(r["destination"],q["destination"]);
        q["destination"]["x"]=json!(destination.x+1);assert_eq!(run(&q,&bits)["error"],"invalidDestination");
    }
}
