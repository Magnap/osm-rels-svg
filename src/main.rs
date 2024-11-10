use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    f64::consts::PI,
    fs::File,
    io::{self, stderr, stdout, BufRead, BufReader, Write},
    ops::Range,
};

use clap::Parser;
use osmpbfreader::{Node, OsmId, OsmObj, OsmPbfReader, Relation, RelationId, Tags, Way, WayId};
use svg::{
    node::element::{path::Data, Group, Path},
    Document,
};

const SCALE: f64 = 6371.0 * 100.0;

#[derive(Debug, Parser)]
#[command(about)]
struct Args {
    #[arg(short, long)]
    data: Box<std::path::Path>,

    #[arg(short, long, default_value = None)]
    output: Option<Box<std::path::Path>>,

    #[arg(short, long, default_value = None)]
    ways: Option<Box<std::path::Path>>,

    #[arg(short, long, default_value = None)]
    relations: Option<Box<std::path::Path>>,
}

#[derive(Debug, PartialEq)]
struct Bound {
    lat: Range<f64>,
    lon: Range<f64>,
}
impl Bound {
    fn new() -> Self {
        Bound {
            lat: 0f64..0f64,
            lon: 0f64..0f64,
        }
    }
    fn is_empty(&self) -> bool {
        *self == Self::new()
    }
    fn update(&mut self, node: &Node) {
        *self = if self.is_empty() {
            Bound {
                lat: node.lat()..node.lat(),
                lon: node.lon()..node.lon(),
            }
        } else {
            Bound {
                lat: self.lat.start.min(node.lat())..self.lat.end.max(node.lat()),
                lon: self.lon.start.min(node.lon())..self.lon.end.max(node.lon()),
            }
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let ways = args.ways.map_or_else(
        || Ok(BTreeSet::new()),
        |w| {
            BufReader::new(File::open(w)?)
                .lines()
                .map(|l| -> Result<_, Box<dyn Error>> { Ok(WayId(l?.parse()?)) })
                .collect()
        },
    )?;
    let relations = args.relations.map_or_else(
        || Ok(BTreeSet::new()),
        |w| {
            BufReader::new(File::open(w)?)
                .lines()
                .map(|l| -> Result<_, Box<dyn Error>> { Ok(RelationId(l?.parse()?)) })
                .collect()
        },
    )?;

    let objs = OsmPbfReader::new(File::open(args.data)?).get_objs_and_deps(|o| match o {
        osmpbfreader::OsmObj::Node(_) => false,
        osmpbfreader::OsmObj::Way(way) => ways.contains(&way.id),
        osmpbfreader::OsmObj::Relation(relation) => relations.contains(&relation.id),
    })?;

    let mut bound = Bound::new();
    let mut svg = Document::new()
        .set("stroke", "#000000")
        .set("stroke-width", 0.0000035 * SCALE)
        .set("stroke-linecap", "round")
        .set("stroke-linejoin", "round");

    for rel in relations {
        if let Some(rel) = objs.get(&OsmId::Relation(rel)) {
            svg = svg.add(relation_to_group(
                &objs,
                &mut bound,
                rel.relation().unwrap(),
            ));
        } else {
            eprintln!("relation {} not found", rel.0);
        }
    }
    for way in ways {
        if let Some(way) = objs.get(&OsmId::Way(way)) {
            svg = svg.add(way_to_path(&objs, &mut bound, way.way().unwrap()));
        } else {
            eprintln!("way {} not found", way.0);
        }
    }

    if !bound.is_empty() {
        let upper_left = project(bound.lat.end.to_radians(), bound.lon.start.to_radians());
        let lower_right = project(bound.lat.start.to_radians(), bound.lon.end.to_radians());
        svg = svg.set(
            "viewBox",
            (
                upper_left.0,
                upper_left.1,
                lower_right.0 - upper_left.0,
                lower_right.1 - upper_left.1,
            ),
        );
    }
    if let Some(output) = args.output {
        svg::save(output, &svg)?;
    } else {
        svg::write(stdout(), &svg)?;
    }

    Ok(())
}

fn relation_to_group(objs: &BTreeMap<OsmId, OsmObj>, bound: &mut Bound, rel: &Relation) -> Group {
    let mut group = set_stroke(Group::new(), &rel.tags).set("id", rel.id.0);
    for r in &rel.refs {
        if let Some(r) = objs.get(&r.member) {
            match r {
                OsmObj::Way(way) => group = group.add(way_to_path(objs, bound, way)),
                OsmObj::Relation(rel) => group = group.add(relation_to_group(objs, bound, rel)),
                OsmObj::Node(_) => {}
            }
        } else {
            eprintln!("ref {:?} of relation {} not found", r.member, rel.id.0)
        }
    }
    group
}

fn way_to_path(objs: &BTreeMap<OsmId, OsmObj>, bound: &mut Bound, way: &Way) -> Path {
    let mut data = Data::new();
    let mut first = true;
    for n in &way.nodes {
        if let Some(n) = objs.get(&OsmId::Node(*n)) {
            let n = n.node().unwrap();
            bound.update(n);
            if first {
                data = data.move_to(project_node(n));
                first = false;
            } else {
                data = data.line_to(project_node(n));
            }
        } else {
            eprintln!("node {} not found", n.0);
        }
    }
    set_stroke(Path::new(), &way.tags)
        .set("id", way.id.0)
        .set("fill", "none")
        .set("d", data)
}

fn set_stroke<N: svg::Node>(mut node: N, tags: &Tags) -> N {
    if let Some(color) = tags.get("colour").filter(|s| s.starts_with('#')) {
        node.assign("stroke", color.to_string())
    };
    node
}

fn project_node(node: &Node) -> (f64, f64) {
    project(node.lat().to_radians(), node.lon().to_radians())
}

fn project(lat: f64, lon: f64) -> (f64, f64) {
    (lon * SCALE, (-lat / 2.0 + PI / 4.0).tan().ln() * SCALE)
}
