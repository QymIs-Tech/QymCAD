//! THE PROBES OF THE BRIDGE ITSELF: what its operations build, asked of the operations directly. They live inside the
//! crate because the operations that build are closed to every other crate - a tool of the window asks the bridge
//! (`ask`) or goes through the door, it does not build.

mod a_fault_of_occt_is_a_refusal;
mod a_boolean_of_touching_bodies_is_refused;
mod a_face_that_did_not_mesh_is_a_hole;
mod a_profile_comes_in_as_it_is;
mod a_mesh_becomes_a_body;
mod a_sheet_has_no_volume;
mod a_union_gives_every_face_a_name_of_its_own;
mod annulus_repro;
mod brep_roundtrip;
mod ffi_guard;
mod fused_profiles_keep_holes;
mod iges;
mod interference_tells_the_truth;
mod monolithic;
mod multiprofile;
mod push_face;
mod remove_faces;
mod shares_geometry;
mod split_body;
mod split_faces;
mod step;
mod step_structure;
mod subdiv_to_kernel;
mod tess_adaptive;
mod the_kernel_says_why_it_refused;
mod thicken;
mod thread;
mod thread_helical;
mod thread_internal_repro;
mod thread_perf;
mod thread_profile_fidelity;
mod valid_gate;
mod what_a_live_shape_costs;
