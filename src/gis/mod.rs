//! Host-free GIS engine: coordinate systems, services and geometry. Nothing in
//! here knows about Open CAD Studio; `crate::emit` turns its output into
//! drawing entities.

pub mod ahn;
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
pub mod servers;
pub mod site;
pub mod terrain;
pub mod vector;
pub mod wfs;
