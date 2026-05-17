// runner.js — CodeLattice WebUI Runner Client (Phase E: Workbench)
var RUNNER=window.RUNNER||{}; window.RUNNER=RUNNER;
RUNNER.base=""; RUNNER.connected=false; RUNNER.profiles=[]; RUNNER.snaps=[];

function rapi(path,opts){
  opts=opts||{}; var url=RUNNER.base+path;
  var init={method:opts.method||"GET",headers:{"Content-Type":"application/json"}};
  if(opts.body)init.body=JSON.stringify(opts.body);
  return fetch(url,init).then(function(r){
    return r.json().then(function(d){
      if(!d.success&&!opts.raw)throw new Error(d.error||r.statusText);
      return d;
    });
  });
}
function runnerCheckHealth(){
  var o=window.location.origin||"http://127.0.0.1:8765"; RUNNER.base=o;
  return rapi("/api/health").then(function(d){
    RUNNER.connected=true; showBadge("runner"); showEl("runner-panel",true); showEl("live-mcp-panel",true);
    runnerLoadProfiles(); runnerLoadLibrary(); pickerLoadQuickRoots();
    if(typeof liveCheckMcp==="function"){liveCheckMcp(); liveLoadTools();}
    return true;
  }).catch(function(){
    RUNNER.connected=false; showBadge("static"); showEl("runner-panel",false); showEl("live-mcp-panel",false);
    var rl=document.getElementById("runner-library-list");
    if(rl)rl.innerHTML='<span class="text-muted text-sm">Start <code>bash scripts/webui-runner.sh --open</code> for local analysis.</span>';
    return false;
  });
}
function showBadge(m){
  var rb=document.getElementById("runner-mode-badge"); var sb=document.getElementById("static-mode-badge");
  if(rb)rb.style.display=m==="runner"?"":"none"; if(sb)sb.style.display=m==="static"?"":"none";}
function showEl(id,v){var e=document.getElementById(id); if(e)e.style.display=v?"":"none";}

// ── Profiles ─────────────────────────────────────────────────────
function runnerLoadProfiles(){
  if(!RUNNER.connected)return;
  rapi("/api/profiles").then(function(d){RUNNER.profiles=d.data||[];renderProfilesList();});
}
function createProfile(){
  if(!RUNNER.connected)return;
  var n=prompt("Profile name:"); if(!n)return;
  var r=prompt("Project root path:"); if(!r)return;
  var l=document.getElementById("runner-lang-select").value;
  rapi("/api/profiles",{method:"POST",body:{name:n,root:r,language:l}}).then(function(d){
    runnerLoadProfiles();
  }).catch(function(e){alert(e.message);});
}
function selectProfile(pid){
  if(!RUNNER.connected)return;
  RUNNER.selectedProfile=pid;
  rapi("/api/profile/"+pid).then(function(d){
    var pf=d.data; if(!pf)return;
    document.getElementById("runner-root-input").value=pf.root||"";
    document.getElementById("runner-lang-select").value=pf.language||"auto";
    runnerLoadLibrary();
    renderProfilesList();
  });
}
function deleteProfile(pid){
  if(!RUNNER.connected||!confirm("Delete profile?"))return;
  rapi("/api/profile/"+pid,{method:"DELETE"}).then(function(){RUNNER.selectedProfile=null;runnerLoadProfiles();runnerLoadLibrary();});
}
function renderProfilesList(){
  var el=document.getElementById("runner-profiles-list"); if(!el)return;
  var pl=RUNNER.profiles; var sp=RUNNER.selectedProfile;
  if(pl.length===0){el.innerHTML='<span class="text-muted text-sm">No profiles. <a href="#" onclick="createProfile();return false;">Create one</a>.</span>';return;}
  el.innerHTML=pl.map(function(p){
    var sel=p.id===sp?'style="border-color:#2563eb;background:#eff6ff;"':"";
    return '<div class="profile-item" '+sel+' style="padding:6px 10px;border:1px solid #e5e7eb;border-radius:4px;margin:2px 0;font-size:0.85em;display:flex;align-items:center;gap:6px;flex-wrap:wrap;">'+
      '<strong style="cursor:pointer;" onclick="selectProfile(&quot;'+p.id+'&quot;)">'+esc(p.name)+'</strong>'+
      '<span class="badge badge-lang">'+esc(p.language)+'</span>'+
      '<span style="color:#9ca3af;font-size:0.75em;">'+esc(p.rootLabel||"")+'</span>'+
      '<span style="color:#9ca3af;">'+p.snapshotCount+' snaps</span>'+
      '<button class="btn btn-sm btn-primary" onclick="selectProfile(&quot;'+p.id+'&quot;);runnerGenForProfile(&quot;'+p.id+'&quot;)" style="margin-left:auto;">Gen</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="deleteProfile(&quot;'+p.id+'&quot;)">×</button>'+
      '</div>';
  }).join("");
}
function runnerGenForProfile(pid){
  if(!RUNNER.connected)return; var st=document.getElementById("runner-status");
  if(st)st.textContent="Generating...";
  rapi("/api/profile/"+pid+"/generate-snapshot",{method:"POST"}).then(function(d){
    if(st)st.textContent="Done: "+(d.data||{}).id; runnerLoadProfiles(); runnerLoadLibrary();
  }).catch(function(e){if(st)st.textContent="Error: "+e.message;});
}

// ── Generate (standalone) ────────────────────────────────────────
function runnerGenerate(){
  if(!RUNNER.connected){alert("Runner not connected.");return;}
  var root=document.getElementById("runner-root-input").value.trim();
  if(!root){alert("Enter project root.");return;}
  var lang=document.getElementById("runner-lang-select").value;
  var pid=RUNNER.selectedProfile||"";
  var st=document.getElementById("runner-status"); if(st)st.textContent="Generating...";
  rapi("/api/generate-snapshot",{method:"POST",body:{root:root,language:lang,full:true,redactRoot:true,profileId:pid}}).then(function(d){
    if(st)st.textContent="Done: "+(d.data||{}).id; runnerLoadLibrary(); runnerLoadProfiles();
  }).catch(function(e){if(st)st.textContent="Error: "+e.message;});
}

// ── Snapshot Library (Phase E enhanced) ──────────────────────────
var libraryFilter={language:"",q:"",sort:"createdAt",order:"desc"};
function runnerLoadLibrary(){
  if(!RUNNER.connected)return;
  var qs="?sort="+libraryFilter.sort+"&order="+libraryFilter.order;
  if(libraryFilter.language)qs+="&language="+libraryFilter.language;
  if(libraryFilter.q)qs+="&q="+encodeURIComponent(libraryFilter.q);
  if(RUNNER.selectedProfile)qs+="&profileId="+RUNNER.selectedProfile;
  rapi("/api/snapshots"+qs).then(function(d){RUNNER.snaps=d.data||[];renderSnapshotLibrary();});
}
function renderSnapshotLibrary(){
  var el=document.getElementById("runner-library-list"); if(!el)return;
  var s=RUNNER.snaps;
  var html='<div style="display:flex;gap:6px;flex-wrap:wrap;margin:8px 0;">'+
    '<input id="lib-search" class="search-input" style="max-width:180px;" placeholder="Search..." oninput="libraryFilter.q=this.value;runnerLoadLibrary()">'+
    '<select id="lib-lang" class="filter-select" onchange="libraryFilter.language=this.value;runnerLoadLibrary()"><option value="">All lang</option>'+SUPPORTED_LANGS.map(function(l){return '<option value="'+l+'">'+l+'</option>';}).join("")+'</select>'+
    '<select class="filter-select" onchange="var p=this.value.split(\":\");libraryFilter.sort=p[0];libraryFilter.order=p[1];runnerLoadLibrary()"><option value="createdAt:desc">Newest</option><option value="createdAt:asc">Oldest</option><option value="symbolCount:desc">Most syms</option><option value="sourceFileCount:desc">Most files</option></select>'+
    '<button class="btn btn-sm btn-secondary" onclick="runnerLoadLibrary()">Refresh</button></div>';
  var SUPPORTED_LANGS=["auto","rust","typescript","c","cpp","python","arkts","cangjie"];
  if(s.length===0){el.innerHTML=html+'<span class="text-muted text-sm">No snapshots.</span>';return;}
  el.innerHTML=html+'<div style="display:flex;gap:6px;flex-wrap:wrap;">'+s.map(function(e){
    var sm=e.summary||{},sc=e.secondary||{};
    return '<div class="snap-card" style="padding:8px 10px;background:#f8fafc;border:1px solid #e5e7eb;border-radius:4px;font-size:0.82em;min-width:220px;">'+
      '<div style="display:flex;justify-content:space-between;align-items:center;">'+
      '<strong title="'+esc(e.id)+'">'+(e.profileName?esc(e.profileName)+" · ":"")+esc(e.createdAt||"").slice(0,16)+'</strong>'+
      '<span class="badge badge-lang">'+esc(e.language||"?")+'</span></div>'+
      '<div style="color:#6b7280;">'+esc(e.rootLabel||"")+' &middot; '+sm.symbolCount+' syms, '+sm.sourceFileCount+' files</div>'+
      '<div style="margin-top:4px;display:flex;gap:4px;flex-wrap:wrap;">'+
      '<button class="btn btn-sm btn-primary" onclick="runnerLoadSnap(&quot;'+escAttr(e.id)+'&quot;)">Load</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerCompareSnap(&quot;'+escAttr(e.id)+'&quot;)">Diff</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerAddTimeline(&quot;'+escAttr(e.id)+'&quot;)">+TL</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerDownloadSnap(&quot;'+escAttr(e.id)+'&quot;)">DL</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="if(confirm(&quot;Delete?&quot;))runnerDeleteSnap(&quot;'+escAttr(e.id)+'&quot;)" style="color:#dc2626;">×</button></div></div>';
  }).join("")+'</div>';
}
function runnerLoadSnap(sid){
  if(!RUNNER.connected)return;
  rapi("/api/snapshot/"+sid).then(function(d){currentSnapshot=d.data;renderAll();
    showEl("loaded-content",true);showEl("welcome-view",false);showEl("error-view",false);
    updateCautionBanner();show("dashboard");}).catch(function(e){alert(e.message);});
}
function runnerCompareSnap(sid){if(!RUNNER.connected)return;
  rapi("/api/snapshot/"+sid).then(function(d){diffSnapshot=d.data;
    document.getElementById("diff-compare-name").textContent="vs "+sid;
    showEl("diff-clear-btn",true);computeAndRenderDiff();show("diff");}).catch(function(e){alert(e.message);});}
function runnerAddTimeline(sid){if(!RUNNER.connected||typeof CTL==="undefined")return;
  rapi("/api/snapshot/"+sid).then(function(d){CTL.timelineSnapshots=CTL.timelineSnapshots||[];
    CTL.timelineSnapshots.push({name:sid+".json",data:d.data});
    CTL.timelineSnapshots.sort(function(a,b){return(a.data.generatedAt||"").localeCompare(b.data.generatedAt||"");});
    document.getElementById("timeline-snapshot-count").textContent=CTL.timelineSnapshots.length+" snaps";
    showEl("timeline-clear-btn",true);CTL.renderTimeline();show("timeline");}).catch(function(e){alert(e.message);});}
function runnerDownloadSnap(sid){if(!RUNNER.connected)return;
  rapi("/api/snapshot/"+sid).then(function(d){var b=new Blob([JSON.stringify(d.data,null,2)],{type:"application/json"});
    var a=document.createElement("a");a.href=URL.createObjectURL(b);a.download="snapshot-"+sid+".json";
    document.body.appendChild(a);a.click();document.body.removeChild(a);URL.revokeObjectURL(a.href);});}
function runnerDeleteSnap(sid){if(!RUNNER.connected)return;
  rapi("/api/snapshot/"+sid,{method:"DELETE"}).then(function(){runnerLoadLibrary();});}
function escAttr(s){return(s||"").replace(/&/g,"&amp;").replace(/"/g,"&quot;").replace(/</g,"&lt;").replace(/>/g,"&gt;");}

// ── Guided Review (Phase E) ──────────────────────────────────────
var GUIDED_SCENARIOS=[
  {id:"onboarding",name:"Project Onboarding",purpose:"Understand project structure, hotspots, entry points.",
   tabs:["dashboard","explore","graph","cleanup"],
   steps:["Inspect Dashboard summary & quality gates","Explore source files & top symbols","Review Graph: nodes/edges/calls","Check Cleanup: dead code candidates & reachability","Export onboarding report"],
   reportTemplate:"onboarding_report"},
  {id:"before_edit",name:"Before Edit Review",purpose:"Assess impact of planned changes before coding.",
   tabs:["dashboard","explore","graph","diff"],
   steps:["Review Dashboard: quality baseline","Explore symbols you plan to change","Check Graph: callers & callees","Load compare snapshot if pre-change snapshot exists","Run Diff to preview impact","Export before_edit report"],
   reportTemplate:"before_edit_risk_report"},
  {id:"after_edit",name:"After Edit Review",purpose:"Verify changes, check for regressions.",
   tabs:["dashboard","diff","timeline","release"],
   steps:["Generate post-edit snapshot","Compare pre/post via Diff","Check Timeline for metric changes","Review Release tab for breaking change risks","Export after_edit report"],
   reportTemplate:"general_snapshot_review"},
  {id:"delete_code",name:"Delete Code Assessment",purpose:"Evaluate safety of removing code/mods.",
   tabs:["explore","graph","cleanup","impact"],
   steps:["Explore symbols to delete: check references","Review Graph: incoming edges","Check Cleanup: dead code candidates (CAUTION: NOT deletion-proof)","Check Impact if available","Manual verification: external API, framework entries","Export delete_code report"],
   reportTemplate:"delete_code_review_report"},
  {id:"release_check",name:"Release Check",purpose:"Pre-release static review.",
   tabs:["dashboard","release","cleanup","timeline"],
   steps:["Verify quality gates pass","Review Release: breaking changes & consistency","Check Cleanup: unreachable, external API","Review Timeline for trend anomalies","Run project tests externally (not in CodeLattice)","Export release review report"],
   reportTemplate:"release_review_report"},
  {id:"legacy_cleanup",name:"Legacy Cleanup",purpose:"Identify unused/untested legacy code.",
   tabs:["explore","graph","cleanup","timeline"],
   steps:["Review Dead Code candidates with cautions","Check Reachability: unreachable symbols","Graph: low-confidence edges review","Timeline: track metric changes over time","Plan manual cleanup with verification steps","Export legacy_cleanup report"],
   reportTemplate:"legacy_cleanup_report"}
];
RUNNER.guidedScenario=null; RUNNER.guidedChecks={};

function guidedRender(){
  var el=document.getElementById("guided-review-panel"); if(!el)return;
  if(!RUNNER.guidedScenario){
    el.innerHTML='<h3>Guided Review</h3><p class="text-muted">Select a scenario to begin a structured code review workflow.</p>'+
      '<div style="display:flex;gap:6px;flex-wrap:wrap;">'+GUIDED_SCENARIOS.map(function(s){
        return '<button class="btn btn-secondary" onclick="guidedSelect(&quot;'+s.id+'&quot;)">'+esc(s.name)+'</button>';
      }).join("")+'</div>';
    return;
  }
  var sc=GUIDED_SCENARIOS.find(function(s){return s.id===RUNNER.guidedScenario;});
  if(!sc)return;
  var checks=RUNNER.guidedChecks[sc.id]||{};
  var allDone=sc.steps.every(function(_,i){return !!checks[i];});
  el.innerHTML='<h3>'+esc(sc.name)+' <span class="badge '+(allDone?"badge-success":"badge-warning")+'">'+
    sc.steps.filter(function(_,i){return !!checks[i];}).length+'/'+sc.steps.length+'</span></h3>'+
    '<p class="text-muted" style="font-size:0.88em;">'+esc(sc.purpose)+'</p>'+
    '<div style="display:flex;gap:4px;flex-wrap:wrap;margin:8px 0;">'+
    sc.tabs.map(function(t){return '<button class="btn btn-sm btn-secondary" onclick="show(&quot;'+t+'&quot;)">'+t+'</button>';}).join("")+' '+
    '<button class="btn btn-sm btn-primary" onclick="guidedReport()">Report</button>'+
    '<button class="btn btn-sm btn-secondary" onclick="guidedReset()">Reset</button>'+
    '<button class="btn btn-sm btn-secondary" onclick="RUNNER.guidedScenario=null;guidedRender();">Back</button></div>'+
    '<div style="font-size:0.88em;">'+sc.steps.map(function(s,i){
      return '<label style="display:flex;align-items:flex-start;gap:6px;padding:4px 0;cursor:pointer;"><input type="checkbox" '+
        (checks[i]?'checked':'')+' onchange="guidedToggle(&quot;'+sc.id+'&quot;,'+i+',this.checked)">'+esc(s)+'</label>';
    }).join("")+'</div>'+
    '<div class="caution-box" style="margin-top:8px;"><strong>Guided Review is a human workflow aid, not proof that checks passed.</strong> Verify externally before release.</div>';
}
function guidedSelect(sid){RUNNER.guidedScenario=sid; guidedRender();}
function guidedToggle(sid,i,v){
  RUNNER.guidedChecks[sid]=RUNNER.guidedChecks[sid]||{}; RUNNER.guidedChecks[sid][i]=v;
  try{localStorage.setItem("codelattice-guided",JSON.stringify(RUNNER.guidedChecks));}catch(e){}
  guidedRender();
}
function guidedReset(){var s=RUNNER.guidedScenario; if(s){RUNNER.guidedChecks[s]={};
  try{localStorage.setItem("codelattice-guided",JSON.stringify(RUNNER.guidedChecks));}catch(e){}}
  guidedRender();}
function guidedReport(){
  if(typeof CTL==="undefined"||!CTL.generateMarkdownReport){show("report");return;}
  var sc=GUIDED_SCENARIOS.find(function(s){return s.id===RUNNER.guidedScenario;});
  CTL.selectedTemplate=sc?sc.reportTemplate:"general_snapshot_review";
  show("report"); CTL.renderReport();
}
try{RUNNER.guidedChecks=JSON.parse(localStorage.getItem("codelattice-guided")||"{}");}catch(e){}

// ── Init ─────────────────────────────────────────────────────────
document.addEventListener("DOMContentLoaded",function(){setTimeout(function(){runnerCheckHealth(); pickerRefresh();},500);});

// ── Project Picker (Phase I) ─────────────────────────────────────
function pickerRefresh(){
  if(RUNNER.connected){
    document.getElementById("picker-runner-hint").style.display="none";
    document.getElementById("picker-hint").textContent=CTL_I18N.t("picker.openProject");
    // Show recent profiles
    rapi("/api/profiles").then(function(d){
      var pl=d.data||[]; var el=document.getElementById("picker-recent-list");
      if(pl.length===0){el.innerHTML='<span class="text-muted text-sm">'+CTL_I18N.t("picker.noRecent")+'</span>'; return;}
      el.innerHTML=pl.slice(0,5).map(function(p){
        return '<div style="padding:4px 0;display:flex;align-items:center;gap:6px;">'+
          '<a href="#" onclick="pickerAnalyzeProfile(&quot;'+escAttr(p.id)+'&quot;);return false;" style="font-weight:500;">'+esc(p.name)+'</a>'+
          '<span class="badge badge-lang">'+esc(p.language)+'</span>'+
          '<span class="text-muted">'+esc(p.rootLabel)+' · '+p.snapshotCount+' snaps</span></div>';
      }).join("");
    });
  }else{
    document.getElementById("picker-runner-hint").style.display="";
  }
}

function pickerPickDirectory(){
  if(!RUNNER.connected){
    alert(CTL_I18N.t("picker.startRunner") + ": " + CTL_I18N.t("picker.startCmd"));
    return;
  }
  var hint=document.getElementById("picker-hint");
  var current=document.getElementById("picker-path-input").value.trim();
  if(hint)hint.textContent=CTL_I18N.t("picker.folderPickerOpening");
  rapi("/api/fs/pick-directory",{method:"POST",body:{currentPath:current}}).then(function(d){
    var path=(d.data||{}).path||"";
    if(!path)throw new Error(CTL_I18N.t("picker.browseUnavailable"));
    pickerSelect(path);
    if(hint)hint.textContent=CTL_I18N.t("picker.selectedFolder");
  }).catch(function(e){
    if(hint)hint.textContent=CTL_I18N.t("picker.folderPickerFallback");
    var fallback=current || "/";
    pickerBrowse(fallback);
  });
}

// 用 runner API 浏览本地文件夹（不经过浏览器上传，数据不离开本机）
function pickerBrowse(path){
  if(!RUNNER.connected){alert(CTL_I18N.t("picker.startRunner") + ": " + CTL_I18N.t("picker.startCmd")); return;}
  var listEl=document.getElementById("picker-browse-list");
  if(!listEl)return;
  listEl.innerHTML='<div style="padding:8px;color:#9ca3af;">'+esc(CTL_I18N.t("picker.browseLoading"))+'</div>';
  rapi("/api/fs/list?path="+encodeURIComponent(path)).then(function(d){
    var dd=d.data;
    if(!dd||!dd.entries){listEl.innerHTML='<div style="padding:8px;color:#dc2626;">'+esc(CTL_I18N.t("picker.browseUnavailable"))+'</div>';return;}
    document.getElementById("picker-path-input").value=dd.path;
    // 面包屑
    var parts=dd.path.split("/").filter(Boolean);
    var bc=parts.map(function(p,i){
      var bp="/"+parts.slice(0,i+1).join("/");
      return '<a href="#" onclick="pickerBrowse(&quot;'+escAttr(bp)+'&quot;);return false;" style="color:#2563eb;">'+esc(p)+'</a>';
    }).join(" / ");
    // 子目录列表
    var dirs=dd.entries.filter(function(e){return e.isDir;});
    listEl.innerHTML='<div style="padding:2px 0;color:#6b7280;">📂 / '+bc+'</div>'+
      '<div style="padding:4px 0;cursor:pointer;color:#059669;font-weight:600;" onclick="pickerSelect(&quot;'+escAttr(dd.path)+'&quot;)">✅ 选定此文件夹</div>'+
      dirs.map(function(e){
        return '<div style="padding:3px 6px;cursor:pointer;color:#2563eb;" onclick="pickerBrowse(&quot;'+escAttr(e.path)+'&quot;)">📁 '+esc(e.name)+'</div>';
      }).join("");
    // 更新快速入口
    document.getElementById("picker-quick-roots").innerHTML=
      '<button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;/&quot;)">📂 /</button> ';
  });
}

function pickerSelect(path){
  document.getElementById("picker-path-input").value=path;
  document.getElementById("picker-browse-list").innerHTML='<div style="padding:8px;color:#059669;">✅ '+esc(path)+' — '+esc(CTL_I18N.t("picker.selectedFolder"))+'</div>';
}

// Runner 连接时加载快速入口
function pickerLoadQuickRoots(){
  if(!RUNNER.connected)return;
  rapi("/api/fs/roots").then(function(d){
    var roots=d.data||[];
    var rootBtns=roots.map(function(r){
      return '<button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;'+escAttr(r.path)+'&quot;)">'+r.icon+' '+esc(r.label)+'</button>';
    }).join(" ");
    document.getElementById("picker-quick-roots").innerHTML=rootBtns+' <button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;/&quot;)">📂 /</button>';
  });
}

function pickerAnalyzePath(){
  var root=document.getElementById("picker-path-input").value.trim();
  if(!root){alert("请输入项目路径"); return;}
  var lang=document.getElementById("picker-lang-select").value;
  document.getElementById("picker-hint").textContent="分析中…";
  rapi("/api/quick-analyze",{method:"POST",body:{root:root,language:lang}}).then(function(d){
    currentSnapshot=d.data.snapshot; renderAll();
    document.getElementById("loaded-content").style.display=""; document.getElementById("welcome-view").style.display="none";
    updateCautionBanner(); show("dashboard"); pickerRefresh();
  }).catch(function(e){alert(e.message); document.getElementById("picker-hint").textContent="输入项目路径开始分析";});
}
function pickerAnalyzeProfile(pid){
  document.getElementById("picker-hint").textContent=CTL_I18N.t("gen.generating");
  rapi("/api/profile/"+pid).then(function(d){
    var p=d.data; document.getElementById("runner-root-input").value=p.root;
    document.getElementById("runner-lang-select").value=p.language;
    return rapi("/api/quick-analyze",{method:"POST",body:{root:p.root,language:p.language}});
  }).then(function(d){
    currentSnapshot=d.data.snapshot; renderAll();
    document.getElementById("loaded-content").style.display=""; document.getElementById("welcome-view").style.display="none";
    updateCautionBanner(); show("dashboard"); pickerRefresh();
  }).catch(function(e){alert(e.message); document.getElementById("picker-hint").textContent=CTL_I18N.t("picker.openProject");});
}
