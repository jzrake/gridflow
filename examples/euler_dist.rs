use clap::{AppSettings, Clap};
use gridiron::automaton;
use gridiron::automaton::{Automaton, Coder};
use gridiron::hydro::euler2d::Primitive;
use gridiron::index_space::range2d;
use gridiron::meshing::GraphTopology;
use gridiron::message::comm::Communicator;
use gridiron::message::null::NullCommunicator;
use gridiron::patch::Patch;
use gridiron::rect_map::Rectangle;
use gridiron::rect_map::RectangleMap;
use gridiron::solvers::euler2d_pcm::{Mesh, PatchUpdate};
use std::collections::HashMap;

/// The initial model
struct Model {}

impl Model {
    fn primitive_at(&self, position: (f64, f64)) -> Primitive {
        let (x, y) = position;
        let r = (x * x + y * y).sqrt();

        if r < 0.24 {
            Primitive::new(1.0, 0.0, 0.0, 1.0)
        } else {
            Primitive::new(0.1, 0.0, 0.0, 0.125)
        }
    }
}

/// The simulation solution state
#[derive(serde::Serialize)]
struct State {
    time: f64,
    iteration: u64,
    primitive: Vec<Patch>,
}

impl State {
    fn new(
        mesh: &Mesh,
        work: &HashMap<Rectangle<i64>, usize>,
        comm: &impl Communicator,
        bs: usize,
    ) -> Self {
        let bs = bs as i64;
        let ni = mesh.size.0 as i64 / bs;
        let nj = mesh.size.1 as i64 / bs;
        let model = Model {};
        let initial_data = |i| model.primitive_at(mesh.cell_center(i)).as_array();
        let primitive = range2d(0..ni, 0..nj)
            .iter()
            .map(|(i, j)| (i * bs..(i + 1) * bs, j * bs..(j + 1) * bs))
            .filter(|rect| work[rect] == comm.rank())
            .map(|rect| Patch::from_vector_function(0, rect, initial_data))
            .collect();

        Self {
            iteration: 0,
            time: 0.0,
            primitive,
        }
    }
}

struct ThisCoder {}

impl Coder for ThisCoder {
    type Type = (
        <PatchUpdate as Automaton>::Key,
        <PatchUpdate as Automaton>::Message,
    );
    fn encode(&self, inst: Self::Type) -> Vec<u8> {
        let mut buffer = Vec::new();
        ciborium::ser::into_writer(&inst, &mut buffer).unwrap();
        buffer
    }
    fn decode(&self, data: Vec<u8>) -> Self::Type {
        ciborium::de::from_reader(data.as_slice()).unwrap()
    }
}

fn work_assignment(
    bs: usize,
    mesh: &Mesh,
    comm: &impl Communicator,
) -> HashMap<Rectangle<i64>, usize> {
    let bs = bs as i64;
    let ni = mesh.size.0 as i64 / bs;
    let nj = mesh.size.1 as i64 / bs;
    let blocks_per_peer = (ni * nj) as usize / comm.size();

    range2d(0..ni, 0..nj)
        .iter()
        .map(|(i, j)| (i * bs..(i + 1) * bs, j * bs..(j + 1) * bs))
        .enumerate()
        .map(|(n, rect)| (rect, n / blocks_per_peer))
        .collect()
}

#[derive(Debug, Clap)]
#[clap(version = "1.0", author = "J. Zrake <jzrake@clemson.edu>")]
#[clap(setting = AppSettings::ColoredHelp)]
struct Opts {
    #[clap(short = 't', long, default_value = "1")]
    num_threads: usize,

    #[clap(
        short = 's',
        long,
        default_value = "serial",
        about = "serial|stupid|rayon"
    )]
    strategy: String,

    #[clap(short = 'n', long, default_value = "1000")]
    grid_resolution: usize,

    #[clap(short = 'b', long, default_value = "100")]
    block_size: usize,

    #[clap(short = 'f', long, default_value = "1")]
    fold: usize,

    #[clap(long, default_value = "0.1")]
    tfinal: f64,
}

fn main() {
    let opts = Opts::parse();
    println!("{:?}", opts);

    let comm = NullCommunicator {};
    let code = ThisCoder {};
    let mesh = Mesh {
        area: (-1.0..1.0, -1.0..1.0),
        size: (opts.grid_resolution, opts.grid_resolution),
    };
    let work = work_assignment(opts.block_size, &mesh, &comm);
    let State {
        mut iteration,
        mut time,
        primitive,
    } = State::new(&mesh, &work, &comm, opts.block_size);

    let primitive_map: RectangleMap<_, _> = primitive
        .into_iter()
        .map(|p| (p.high_resolution_rect(), p))
        .collect();
    let dt = mesh.cell_spacing().0 * 0.1;
    let edge_list = primitive_map.adjacency_list(1);
    let primitive: Vec<_> = primitive_map.into_iter().map(|(_, prim)| prim).collect();

    println!("num blocks .... {}", primitive.len());
    println!("num threads ... {}", opts.num_threads);
    println!("");

    let mut task_list: Vec<_> = primitive
        .into_iter()
        .map(|patch| PatchUpdate::new(patch, mesh.clone(), dt, None, &edge_list))
        .collect();

    if opts.grid_resolution % opts.block_size != 0 {
        eprintln!("Error: block size must divide the grid resolution");
        return;
    }

    while time < opts.tfinal {
        let start = std::time::Instant::now();

        for _ in 0..opts.fold {
            task_list = automaton::execute_dist(&comm, &code, &work, task_list);
            iteration += 1;
            time += dt;
        }

        let step_seconds = start.elapsed().as_secs_f64() / opts.fold as f64;
        let mzps = mesh.total_zones() as f64 / 1e6 / step_seconds;

        println!(
            "[{}] t={:.3} Mzps={:.2} ({:.2}-thread)",
            iteration,
            time,
            mzps,
            mzps / opts.num_threads as f64
        );
    }

    let primitive = task_list
        .into_iter()
        .map(|block| block.primitive())
        .collect();
    let state = State {
        iteration,
        time,
        primitive,
    };

    let file = std::fs::File::create(format! {"state.{:04}.cbor", comm.rank()}).unwrap();
    let mut buffer = std::io::BufWriter::new(file);
    ciborium::ser::into_writer(&state, &mut buffer).unwrap();
}