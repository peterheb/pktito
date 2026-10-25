//! What the window (main.rs) and the command line (bin/pktito-cli.rs)
//! share: reading audio and video, the models and their download, the
//! engine, transcribing, the recent list's transcripts, the formats a
//! transcript is saved in, and the window's words; and, the window's alone,
//! the server its player gets the audio from on Linux. None of it knows of
//! the window. The Android app (android/) takes it through android.rs.

#[cfg(target_os = "android")]
pub mod android;
pub mod audio;
pub mod decode;
pub mod download;
pub mod format;
pub mod model;
pub mod mp3;
pub mod mp4;
pub mod pktito;
pub mod recent;
#[cfg(unix)]
pub mod serve;
pub mod strings;
pub mod transcribe;
