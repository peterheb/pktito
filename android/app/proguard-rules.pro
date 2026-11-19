# R8's rules for the release build beyond Android's own
# (proguard-android-optimize.txt, which keeps native methods and the
# classes that have them, so Engine's names, which the native library's
# functions are named after).

# The native library calls Engine.Progress's two methods by their names and
# signatures on whatever implements it (src-tauri/src/android.rs, transcribe:
# "audio" (DD[B)V and "update" (DLjava/lang/String;)V)
-keepclassmembers class * implements net.pktito.app.Engine$Progress {
    public void reading(double);
    public void audio(double, double, byte[]);
    public void update(double, java.lang.String);
}

# and Engine.Heard's, while speakers are found (android.rs, find:
# "decoding" (D)V and "done" (DLjava/lang/String;)V)
-keepclassmembers class * implements net.pktito.app.Engine$Heard {
    public void decoding(double);
    public void done(double, java.lang.String);
}

# and reads Engine.Kept's fields by their names (android.rs, kept_as)
-keepclassmembers class net.pktito.app.Engine$Kept {
    java.lang.String dir;
    java.lang.String id;
    java.lang.String what;
    long size;
    long modified;
}

# and Engine.Download's one (android.rs, download: "state" (Ljava/lang/String;)V)
-keepclassmembers class * implements net.pktito.app.Engine$Download {
    public void state(java.lang.String);
}
