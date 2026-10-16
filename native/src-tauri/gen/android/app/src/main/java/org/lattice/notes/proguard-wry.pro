# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class org.lattice.notes.* {
  native <methods>;
}

-keep class org.lattice.notes.WryActivity {
  public <init>(...);

  void setWebView(org.lattice.notes.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class org.lattice.notes.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class org.lattice.notes.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class org.lattice.notes.RustWebChromeClient,org.lattice.notes.RustWebViewClient {
  public <init>(...);
}
