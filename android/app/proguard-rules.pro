# R8's rules for the release build beyond Android's own
# (proguard-android-optimize.txt, which keeps native methods and the
# classes that have them, so Engine's names, which the native library's
# functions are named after).

# The native library calls Engine.Progress's two methods by their names and
# signatures on whatever implements it (src-tauri/src/android.rs, transcribe:
# "audio" (DD[B)V and "update" (DLjava/lang/String;)V)
-keepclassmembers class * implements net.pktito.app.Engine$Progress {
    public void audio(double, double, byte[]);
    public void update(double, java.lang.String);
}
