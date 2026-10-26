# Google Play

What Play asks of the Android app before it can be listed, and pktito's
answers, for a personal developer account in Peter's name. The answers
describe the app as it is on the `android` branch; where one waits on
something still to come, it says so.

## The build

```sh
NDK=/path/to/android-ndk android/native.sh          # the engine and the Rust core
cd android && ./gradlew bundleRelease               # app/build/outputs/bundle/release/app-release.aab
python3 android/notices.py --check                  # the licenses list is the build's
```

- R8 shrinks the release (4.7 MB APK, 4.6 MB bundle); `proguard-rules.pro`
  keeps the two methods the native library calls by name.
- Native code for arm64-v8a and x86_64 only (`abiFilters`), each library 16 KB
  aligned, as Play requires of apps for Android 15 and later.
- targetSdk 36, minSdk 29 (Android 10). `versionCode` goes up with every upload.
- Play App Signing keeps the app's signing key. The bundle is signed with an
  upload key that Peter makes and keeps; it is not in this repo, and the
  release build is unsigned until it is set up.

## Store listing

- **Name:** pktito
- **Icon:** `icon-512.png` (from `android/icon.py`)
- **Category:** Productivity
- **Website:** https://pktito.net · **Privacy policy:** https://pktito.net/privacy
- **Contact email:** peter.hebert@pktito.net, the account's support email, which the
  privacy policy gives too
- **Still to make:** the 1024 × 500 feature graphic, and phone screenshots
  (2 to 8, from the Galaxy S25); Chromebook ones from the Googlebook are optional.

## App content

**Privacy policy.** https://pktito.net/privacy, once the site carries its
Android paragraph (fc3d142, not deployed). The app has the same words in
About, Privacy.

**Ads:** none. **App access:** everything works without an account or
login. **Target audience:** 13 and over (nothing in it is for children, and
choosing under 13 brings the Families policy). **Content rating:** the IARC
questionnaire's answers should all come out no: a utility that shows the
user's own recording as text, shares nothing between users, and has no
purchases.

**Data safety.** Google counts as collected what leaves the device
([Data safety](https://support.google.com/googleplay/android-developer/answer/10787469)).
The recordings and transcripts never do: the engine runs on the device, and
files come and go only through the system's pickers, Open with and Share.
The one thing that leaves is the model download's request to
models.pktito.net, from the setup screen or the model menu: the IP address
and user agent the server logs, for security and to count downloads, as the
policy says. A model file of one's own is copied into the app from the
system's picker and never leaves the device. Google's form has no
IP-address type, and its guidance is to disclose IP addresses "based on
their particular usage". No location is
inferred and no profile kept, so the leaning answer is **no data collected
or shared**, with that reasoning on file. That is Peter's to decide when he
fills in the form.

**Foreground service.** Each type the manifest declares needs a description,
what deferring it would cost the user, a use case from Play's list and a
video ([foreground service requirements](https://support.google.com/googleplay/android-developer/answer/13392821)).
The transcription uses mediaProcessing on Android 15 and later, and dataSync
before it, where mediaProcessing does not exist. The model download is a
user-initiated data transfer job on Android 14 and later, which that page
recommends over dataSync for a transfer the user asks for, and which is not
a foreground service; on Android 10 to 13, which have no such jobs, it is a
dataSync foreground service, so dataSync has a second use case:

| | mediaProcessing | dataSync |
|---|---|---|
| Use case | Media transcoding | Local processing: Other |
| Description | pktito transcribes an audio or video file the user chose, on the device: it turns its speech into text. An hour of audio takes about 50 seconds on a Snapdragon X Elite laptop (69.8 times real time, AliceBench), and longer on a phone. The user starts it with a tap and sees its progress in a notification, which opens the app, where Stop is. It never starts on its own. | The same transcription, on Android 10 to 14. |
| If deferred | The user waits for a transcript they asked for. Interrupted, it ends where it got to, and finishing it means transcribing the recording again: the app keeps nothing to resume from. | The same. |

| | dataSync, the model download (Android 10 to 13) |
|---|---|
| Use case | Network transfer: Upload or download |
| Description | pktito downloads the speech model the user chose (624 or 648 MB, once) from models.pktito.net, when they tap Download on the setup screen or choose a model in the model menu. Its progress is in a notification; the app has Pause and Cancel. It never starts on its own. |
| If deferred | The app cannot transcribe until it has a model. Interrupted, the download keeps what it has and goes on from there. |

The videos: for the transcription, choose a recording, see the transcription
start with its notification, switch to another app while the notification
counts up, come back to the finished transcript; for the download, tap
Download the English model on the setup screen and watch it come, in the
app and in its notification. Recorded on the S25, as Peter chooses.

**Generative AI.** Play's AI-generated content policy covers apps that make
new content from prompts: chatbots, image generators, made-up voices or
videos of people ([AI-generated content](https://support.google.com/googleplay/android-developer/answer/14094294)).
pktito writes down what a recording says. It takes no prompt and generates
nothing, so the policy should not apply. If the Console asks, the answer is
on-device speech recognition.

**Downloaded models.** Play forbids downloading executable code from outside
Play. A .pkf file is the model's weights, which the engine reads as data; it
runs no code from it.

## The closed test

A personal account made after November 13, 2023 gets production access only
after a closed test: at least 12 testers opted in continuously for 14 days,
then Apply for production in the Console, which Google usually reviews in
seven days or less
([testing requirements](https://support.google.com/googleplay/android-developer/answer/14151465)).
The testers install from Play, so they cannot be given a model by adb: the
build they test downloads one from the setup screen, as the desktop's does.
