// SPDX-License-Identifier: MIT OR Apache-2.0
// Workload: declarative (static CDP automation-signal payloads; no I/O)
//! CDP scripts injected via `Page.addScriptToEvaluateOnNewDocument`.
//!
//! Split from `browser` for single-responsibility (Pass 35 componentization):
//! this module owns only **automation-signal mitigation** JS — no process launch.
//!
//! ## Policy (ADR-0022 / GraphRAG anti-bot)
//!
//! - **Allowed:** hide CDP automation tells (`navigator.webdriver`, empty plugins,
//!   missing `window.chrome`, headless outer size, permissions quirks, DevTools
//!   WebSocket leak).
//! - **Forbidden:** synthetic **hardware fingerprint spoofing** (canvas noise,
//!   WebGL GPU lies, AudioContext noise, forced `hardwareConcurrency` /
//!   `deviceMemory` / `colorDepth`). Static spoofs become a shared automation
//!   fingerprint that Cloudflare Bot Management can score and block.
//! - Production TLS is the **native Chrome process** stack (ADR-0016) — not a
//!   marketed “fingerprint feature”. Residual HTTP uses rustls (ADR-0021).

/// Automation-signal mitigation scripts for CDP `Page.addScriptToEvaluateOnNewDocument`.
///
/// Layer A — JS environment automation tells:
///   `webdriver`, plugins, `window.chrome`, vendor, outer dimensions, permissions,
///   iframe `contentWindow` chrome mirror.
///
/// Layer B — CDP leak prevention (GAP-WS-076):
///   block page JS from opening DevTools WebSockets to localhost.
///
/// **Does not** spoof canvas / WebGL / Audio / hardware concurrency (ADR-0022).
pub(crate) const STEALTH_SCRIPTS: &str = concat!(
    // --- Layer A: basic JS environment (automation signals only) ---
    "Object.defineProperty(navigator,'webdriver',{get:()=>undefined});",
    "Object.defineProperty(navigator,'maxTouchPoints',{get:()=>0});",
    "Object.defineProperty(navigator,'vendor',{get:()=>'Google Inc.'});",
    // --- Layer A+: chrome object emulation (missing in many automation builds) ---
    "window.chrome={runtime:{PlatformOs:{MAC:'mac',WIN:'win',ANDROID:'android',CROS:'cros',LINUX:'linux',OPENBSD:'openbsd'},PlatformArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},PlatformNaclArch:{ARM:'arm',X86_32:'x86-32',X86_64:'x86-64',MIPS:'mips',MIPS64:'mips64'},RequestUpdateCheckStatus:{THROTTLED:'throttled',NO_UPDATE:'no_update',UPDATE_AVAILABLE:'update_available'},OnInstalledReason:{INSTALL:'install',UPDATE:'update',CHROME_UPDATE:'chrome_update',SHARED_MODULE_UPDATE:'shared_module_update'},OnRestartRequiredReason:{APP_UPDATE:'app_update',OS_UPDATE:'os_update',PERIODIC:'periodic'},connect:function(){},sendMessage:function(){}},app:{isInstalled:false,InstallState:{INSTALLED:'installed',DISABLED:'disabled',NOT_INSTALLED:'not_installed'},RunningState:{RUNNING:'running',CANNOT_RUN:'cannot_run',READY_TO_RUN:'ready_to_run'}},loadTimes:function(){return{requestTime:Date.now()/1000,startLoadTime:Date.now()/1000,commitLoadTime:Date.now()/1000,finishDocumentLoadTime:Date.now()/1000,finishLoadTime:Date.now()/1000,firstPaintTime:Date.now()/1000,firstPaintAfterLoadTime:0,navigationType:'Other',wasFetchedViaSpdy:true,wasNpnNegotiated:true,npnNegotiatedProtocol:'h2',wasAlternateProtocolAvailable:false,connectionInfo:'h2'}},csi:function(){return{onloadT:Date.now(),startE:Date.now(),pageT:0,tran:15}}};",
    // --- Layer A+: realistic PluginArray (empty plugins = automation tell) ---
    "(function(){function P(n,d,f,m){this.name=n;this.description=d;this.filename=f;this.length=m.length;for(var i=0;i<m.length;i++)this[i]=m[i]}var p=[new P('Chrome PDF Plugin','Portable Document Format','internal-pdf-viewer',[{type:'application/x-google-chrome-pdf',suffixes:'pdf',description:'Portable Document Format'}]),new P('Chrome PDF Viewer','','mhjfbmdgcfjbbpaeojofohoefgiehjai',[{type:'application/pdf',suffixes:'pdf',description:''}]),new P('Native Client','','internal-nacl-plugin',[{type:'application/x-nacl',suffixes:'',description:'Native Client Executable'},{type:'application/x-pnacl',suffixes:'',description:'Portable Native Client Executable'}])];Object.defineProperty(navigator,'plugins',{get:function(){return p}});Object.defineProperty(navigator,'mimeTypes',{get:function(){return p.reduce(function(a,pl){for(var i=0;i<pl.length;i++)a.push(pl[i]);return a},[])}})})()",
    ";",
    // --- Layer A+: window outer dimensions (0 in some headless builds = detection) ---
    "Object.defineProperty(window,'outerHeight',{get:function(){return window.innerHeight+85}});",
    "Object.defineProperty(window,'outerWidth',{get:function(){return window.innerWidth+15}});",
    // --- Layer A+: Permissions API (notifications quirks in headless) ---
    "(function(){if(typeof Permissions!=='undefined'){var o=Permissions.prototype.query;Permissions.prototype.query=function(p){if(p&&p.name==='notifications')return Promise.resolve({state:Notification.permission==='denied'?'denied':'prompt',onchange:null});return o.apply(this,arguments)}}})()",
    ";",
    // --- Layer A+: iframe contentWindow chrome mirror ---
    "(function(){try{var F=HTMLIFrameElement.prototype;var d=Object.getOwnPropertyDescriptor(F,'contentWindow');if(d&&d.get){var o=d.get;Object.defineProperty(F,'contentWindow',{get:function(){var w=o.call(this);if(w&&w.chrome)w.chrome=window.chrome;return w}})}}catch(e){}})()",
    ";",
    // --- Layer B: CDP leak prevention (GAP-WS-076) ---
    "(function(){var O=window.WebSocket;window.WebSocket=function(u,p){if(u&&typeof u==='string'&&(u.includes('/devtools/')||u.includes('ws://127.0.0.1')))return{close:function(){},send:function(){},addEventListener:function(){},readyState:3};return p?new O(u,p):new O(u)};window.WebSocket.prototype=O.prototype;window.WebSocket.CONNECTING=0;window.WebSocket.OPEN=1;window.WebSocket.CLOSING=2;window.WebSocket.CLOSED=3})();",
    // Extended Permissions API (clipboard, geolocation, camera, microphone)
    "(function(){if(typeof Permissions!=='undefined'){var o=Permissions.prototype.query;Permissions.prototype.query=function(p){if(p&&p.name){var s={notifications:'prompt',geolocation:'prompt','clipboard-read':'prompt','clipboard-write':'granted',camera:'prompt',microphone:'prompt'};if(s[p.name]!==undefined)return Promise.resolve({state:s[p.name],onchange:null})}return o.apply(this,arguments)}}})()",
    ";",
);

/// Platform-specific `navigator.platform` script (must match host OS / Chrome UA).
/// Injected alongside [`STEALTH_SCRIPTS`] for UA↔platform coherence (not hardware spoof).
#[cfg(target_os = "linux")]
pub(crate) const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Linux x86_64'});";
#[cfg(target_os = "macos")]
pub(crate) const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'MacIntel'});";
#[cfg(target_os = "windows")]
pub(crate) const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Win32'});";
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) const STEALTH_PLATFORM_SCRIPT: &str =
    "Object.defineProperty(navigator,'platform',{get:()=>'Linux x86_64'});";
