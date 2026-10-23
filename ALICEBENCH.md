# AliceBench results

Every AliceBench v1 run (scripts/alicebench.sh, .ps1): LibriVox's _Alice's
Adventures in Wonderland_, twelve chapters, 2:58:51 of 16 kHz FLAC, with
the English model (parakeet-tdt-0.6b-v2). A dated record: append, don't
rewrite. Which of these the site shows is decided there, not here.

"All 12" means every chapter's transcript matched the reference, so the
build computes what every other build does.

| Date       | Machine                                          | OS                 | pktito | Code path | Threads | × real time | Whole book | Transcripts | Notes                                                                             |
| ---------- | ------------------------------------------------ | ------------------ | ------ | --------- | ------- | ----------- | ---------- | ----------- | --------------------------------------------------------------------------------- |
| 2026-10-23 | Mac mini, M4 Pro                                 | macOS 27           | 0.8.4  | neon-dp   | 12      | 101.8       | 1:45       | all 12      | a stray Python process held one core                                              |
| 2026-10-23 | Mac mini, M4 Pro                                 | macOS 27           | 0.8.4  | neon-dp   | 12      | 106.1       | 1:41       | all 12      | in use: other apps open, load 5 to 12; chapters 101–114×                          |
| 2026-10-23 | NVIDIA DGX Spark (GB10: 10 Cortex-X925, 10 A725) | Ubuntu 24.04       | 0.8.5  | neon-dp   | 20      | 113.0       | 1:35       | all 12      | quiet; a vLLM model loaded, GPU idle; chapters 108–115×                           |
| 2026-10-23 | Raspberry Pi 5, 4 GB (Cortex-A76 ×4)             | Raspberry Pi OS 13 | 0.8.5  | neon-dp   | 4       | 14.4        | 12:25      | all 12      | quiet; 47.7 to 67.5 °C, never throttled; chapters 14.1–14.6×                      |
| 2026-10-23 | AMD Ryzen 9 9950X (Zen 5, 16 cores)              | Windows 11         | 0.8.5  | avx2-vnni | 16      | 212.6       | 0:50       | all 12      | quiet (pinfer paused); chapter 1 186×, the rest 211–218×; 62 s wall with decoding |
