# R8 rules for the release build (see docs/mobile.md, Phase 7 (Android)).

# JNA: native code looks up classes, fields and methods by name (Structure fields, Callback.invoke,
# Native.register). Nothing in it may be renamed or removed.
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { *; }
-dontwarn java.awt.**

# UniFFI bindings (generated, package uniffi.quest_mobile): JNA binds the `external fun`s of the Library
# objects by name, reads RustBuffer/RustCallStatus fields by reflection and calls the callback
# interfaces (QuestObserver, foreign futures) through their `callback` methods.
-keep class uniffi.** { *; }

# rustls-platform-verifier: the Rust core calls its Kotlin half over JNI by class and method name.
-keep, includedescriptorclasses class org.rustls.platformverifier.** { *; }

# Glance stores an ActionCallback's class name in the widget's PendingIntent and creates it by
# reflection with its no-argument constructor (ToggleTaskAction, the widget's check).
-keep class * implements androidx.glance.appwidget.action.ActionCallback { <init>(); }
