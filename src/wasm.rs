//! What the WebAssembly modules export. Each file here that computes for a
//! caller has its HTTP twin in `server/src/api/`: same name, same computation
//! through `shared`, another transport. The map's exports have none — a
//! frame is for a canvas.

pub mod chapter;
pub mod els;
pub mod fault;
pub mod landing;
pub mod map;
