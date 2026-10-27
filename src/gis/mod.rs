//! Host-free GIS engine: coordinate systems, services and geometry. Nothing in
//! here knows about Open CAD Studio; `crate::emit` turns its output into
//! drawing entities.

pub mod ahn;
pub mod bag3d;
pub mod bgt;
pub mod caps;
pub mod crs;
pub mod geocode;
pub mod http;
pub mod klic;
pub mod ogcapi;
pub mod osm;
pub mod pointcloud;
pub mod raster;
pub mod rasterbestand;
pub mod servers;
pub mod site;
pub mod vector;
pub mod wfs;
