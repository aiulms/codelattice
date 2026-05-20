// runner.js — CodeLattice WebUI Runner Client (Phase E: Workbench)
var RUNNER=window.RUNNER||{}; window.RUNNER=RUNNER;
RUNNER.base=""; RUNNER.connected=false; RUNNER.profiles=[]; RUNNER.snaps=[];
var SUPPORTED_LANGS=["auto","rust","typescript","c","cpp","python","shell","arkts","cangjie"];
var LAST_SNAPSHOT_KEY="codelattice.webui.lastSnapshotId";
var LAST_TAB_KEY="codelattice.webui.lastTab";
function tr(k,p){return window.CTL_I18N?CTL_I18N.t(k,p):k;}

function rapi(path,opts){
  opts=opts||{}; var url=RUNNER.base+path;
  var init={method:opts.method||"GET",headers:{"Content-Type":"application/json"}};
  if(opts.body)init.body=JSON.stringify(opts.body);
  return fetch(url,init).then(function(r){
    return r.json().then(function(d){
      if(!d.success&&!opts.raw){
        var ex=new Error(d.error||r.statusText);
        ex.hint=d.hint||"";
        ex.status=d.status||r.status;
        ex.response=d;
        throw ex;
      }
      return d;
    });
  });
}
function runnerCheckHealth(){
  var o=window.location.origin||"http://127.0.0.1:8765"; RUNNER.base=o;
  return rapi("/api/health").then(function(d){
    RUNNER.connected=true; showBadge("runner"); showEl("runner-panel",true); showEl("live-mcp-panel",true);
    runnerLoadProfiles(); runnerLoadLibrary(); pickerLoadQuickRoots(); runnerLoadQuickRoots(); pickerRefresh();
    if(typeof liveCheckMcp==="function"){liveCheckMcp(); liveLoadTools();}
    setTimeout(restoreWorkbenchSnapshot, 150);
    return true;
  }).catch(function(){
    RUNNER.connected=false; showBadge("static"); showEl("runner-panel",false); showEl("live-mcp-panel",false);
    var rl=document.getElementById("runner-library-list");
    if(rl)rl.innerHTML='<span class="text-muted text-sm">'+esc(tr("runner.startHint"))+' <code>bash scripts/webui-runner.sh --open</code></span>';
    return false;
  });
}
function showBadge(m){
  var rb=document.getElementById("runner-mode-badge"); var sb=document.getElementById("static-mode-badge");
  if(rb)rb.style.display=m==="runner"?"":"none"; if(sb)sb.style.display=m==="static"?"":"none";}
function showEl(id,v){var e=document.getElementById(id); if(e)e.style.display=v?"":"none";}

function currentUrl(){
  try{return new URL(window.location.href);}catch(e){return null;}
}
function getUrlParam(name){
  var u=currentUrl(); return u?u.searchParams.get(name):"";
}
function rememberWorkbenchSnapshot(sid, tab){
  if(!sid)return;
  try{localStorage.setItem(LAST_SNAPSHOT_KEY,sid);}catch(e){}
  if(tab)try{localStorage.setItem(LAST_TAB_KEY,tab);}catch(e){}
  var u=currentUrl();
  if(u&&window.history&&window.history.replaceState){
    u.searchParams.set("snapshot",sid);
    if(tab)u.searchParams.set("tab",tab);
    window.history.replaceState(null,"",u.toString());
  }
}
function rememberWorkbenchTab(tab){
  if(!tab)return;
  try{localStorage.setItem(LAST_TAB_KEY,tab);}catch(e){}
  var u=currentUrl();
  if(u&&u.searchParams.get("snapshot")&&window.history&&window.history.replaceState){
    u.searchParams.set("tab",tab);
    window.history.replaceState(null,"",u.toString());
  }
}
function openWorkbenchSnapshot(snapshot, sid, opts){
  opts=opts||{};
  currentSnapshot=snapshot;
  renderAll();
  showEl("loaded-content",true);
  showEl("welcome-view",false);
  showEl("error-view",false);
  updateCautionBanner();
  var tab=opts.tab||getUrlParam("tab")||localStorage.getItem(LAST_TAB_KEY)||"dashboard";
  show(tab);
  if(opts.remember!==false)rememberWorkbenchSnapshot(sid,tab);
}
function restoreWorkbenchSnapshot(){
  if(RUNNER.restoreAttempted||window.currentSnapshot)return;
  RUNNER.restoreAttempted=true;
  var sid=getUrlParam("snapshot")||localStorage.getItem(LAST_SNAPSHOT_KEY)||"";
  if(!sid)return;
  runnerLoadSnap(sid,{remember:false,silent:true,tab:getUrlParam("tab")||localStorage.getItem(LAST_TAB_KEY)||"dashboard"}).catch(function(){
    try{localStorage.removeItem(LAST_SNAPSHOT_KEY);}catch(e){}
  });
}

// ── Profiles ─────────────────────────────────────────────────────
function runnerLoadProfiles(){
  if(!RUNNER.connected)return;
  rapi("/api/profiles").then(function(d){RUNNER.profiles=d.data||[];renderProfilesList();});
}
function createProfile(){
  if(!RUNNER.connected)return;
  var n=prompt(tr("profile.namePrompt")); if(!n)return;
  var r=prompt(tr("profile.rootPrompt")); if(!r)return;
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
  if(!RUNNER.connected||!confirm(tr("profile.delete")))return;
  rapi("/api/profile/"+pid,{method:"DELETE"}).then(function(){RUNNER.selectedProfile=null;runnerLoadProfiles();runnerLoadLibrary();});
}
function renderProfilesList(){
  var el=document.getElementById("runner-profiles-list"); if(!el)return;
  var pl=RUNNER.profiles; var sp=RUNNER.selectedProfile;
  if(pl.length===0){el.innerHTML='<span class="text-muted text-sm">'+esc(tr("profile.noProfiles"))+' <a href="#" onclick="createProfile();return false;">'+esc(tr("profile.createOne"))+'</a>.</span>';return;}
  el.innerHTML=pl.map(function(p){
    var sel=p.id===sp?'style="border-color:#2563eb;background:#eff6ff;"':"";
    return '<div class="profile-item" '+sel+' style="padding:6px 10px;border:1px solid #e5e7eb;border-radius:4px;margin:2px 0;font-size:0.85em;display:flex;align-items:center;gap:6px;flex-wrap:wrap;">'+
      '<strong style="cursor:pointer;" onclick="selectProfile(&quot;'+p.id+'&quot;)">'+esc(p.name)+'</strong>'+
      '<span class="badge badge-lang">'+esc(p.language)+'</span>'+
      '<span style="color:#9ca3af;font-size:0.75em;">'+esc(p.rootLabel||"")+'</span>'+
      '<span style="color:#9ca3af;">'+p.snapshotCount+' '+esc(tr("profile.snaps"))+'</span>'+
      '<button class="btn btn-sm btn-primary" onclick="selectProfile(&quot;'+p.id+'&quot;);runnerGenForProfile(&quot;'+p.id+'&quot;)" style="margin-left:auto;">'+esc(tr("profile.gen"))+'</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="deleteProfile(&quot;'+p.id+'&quot;)">×</button>'+
      '</div>';
  }).join("");
}
function runnerGenForProfile(pid){
  if(!RUNNER.connected)return; var st=document.getElementById("runner-status");
  if(st)st.textContent=tr("gen.generating");
  rapi("/api/profile/"+pid+"/generate-snapshot",{method:"POST"}).then(function(d){
    var sid=(d.data||{}).id;
    if(st)st.textContent=tr("gen.done")+": "+sid; runnerLoadProfiles(); runnerLoadLibrary();
    if(sid)runnerLoadSnap(sid,{tab:"dashboard"});
  }).catch(function(e){if(st)st.textContent=tr("gen.error")+": "+e.message;});
}

// ── Generate (standalone) ────────────────────────────────────────
function runnerGenerate(){
  if(!RUNNER.connected){alert(tr("runner.notConnected"));return;}
  var root=document.getElementById("runner-root-input").value.trim();
  if(!root){alert(tr("error.missingRoot"));return;}
  var lang=document.getElementById("runner-lang-select").value;
  var pid=RUNNER.selectedProfile||"";
  var st=document.getElementById("runner-status"); if(st)st.textContent=tr("gen.generating");
  return analyzeAfterInventory(root,lang,"runner",function(targetRoot,targetLang){
    if(st)st.textContent=tr("gen.generating");
    return rapi("/api/generate-snapshot",{method:"POST",body:{root:targetRoot,language:targetLang,full:true,redactRoot:true,profileId:pid}}).then(function(d){
      var sid=(d.data||{}).id;
      if(st)st.textContent=tr("gen.done")+": "+sid; runnerLoadLibrary(); runnerLoadProfiles();
      if(sid)runnerLoadSnap(sid,{tab:"dashboard"});
    }).catch(function(e){showGenerationError(e, "runner");});
  });
}

function showGenerationError(e, where){
  var msg=tr("gen.error")+": "+(e.message||"snapshot generation failed");
  var hint=e.hint||"";
  var full=msg+(hint?" — "+hint:"")+" "+tr("gen.showingPrevious");
  var st=document.getElementById("runner-status");
  if(st)st.textContent=full.length>260?full.slice(0,260)+"…":full;
  var target=where==="picker"?"picker-browse-list":"runner-browse-list";
  var listEl=document.getElementById(target);
  if(listEl){
    var details=listEl.closest&&listEl.closest("details");
    if(details)details.open=true;
    var candidates=extractProjectCandidates(hint);
    var candidateHtml=candidates.length?renderProjectCandidates(candidates, where):"";
    listEl.innerHTML='<div style="padding:10px;color:#b91c1c;background:#fef2f2;border:1px solid #fecaca;border-radius:6px;white-space:pre-wrap;">'+
      esc(msg)+(hint?'\n\n'+esc(hint):'')+'\n\n'+esc(tr("gen.showingPrevious"))+'</div>'+candidateHtml;
  }
}

function extractProjectCandidates(hint){
  if(!hint)return[];
  return hint.split(/\n/).map(function(line){
    var m=line.match(/^- (\/.+?) \((.+?)\)\s*$/);
    if(!m)return null;
    var langs=m[2].split(",").map(function(s){return s.trim();}).filter(Boolean);
    return {path:m[1],languages:langs,unsupported:langs.some(function(l){return l.indexOf("unsupported:")===0;})};
  }).filter(Boolean).slice(0,12);
}
function candidateLang(c){
  var lang=(c.languages||[]).find(function(l){return l.indexOf("unsupported:")!==0;})||"auto";
  if(lang==="c/cpp")return"cpp";
  return lang;
}
function renderProjectCandidates(candidates, where){
  var rows=candidates.map(function(c){
    var lang=candidateLang(c);
    var unsupported=c.unsupported&&!c.languages.some(function(l){return l.indexOf("unsupported:")!==0;});
    var label=c.unsupported?tr("gen.unsupportedModule"):tr("gen.useCandidate");
    var action=where==="picker"?"pickerUseCandidate":"runnerUseCandidate";
    return '<button class="candidate-project '+(unsupported?'disabled':'')+'" '+(unsupported?'disabled':'onclick="'+action+'(&quot;'+escAttr(c.path)+'&quot;,&quot;'+escAttr(lang)+'&quot;)"')+'>'+
      '<span class="candidate-path">'+esc(c.path)+'</span>'+
      '<span class="candidate-lang">'+esc(c.languages.join(", "))+'</span>'+
      '<strong>'+esc(label)+'</strong>'+
    '</button>';
  }).join("");
  return '<div class="candidate-projects"><div class="candidate-title">'+esc(tr("gen.candidateProjects"))+'</div>'+rows+'</div>';
}
function runnerUseCandidate(path, lang){
  var input=document.getElementById("runner-root-input");
  var sel=document.getElementById("runner-lang-select");
  if(input)input.value=path;
  if(sel&&SUPPORTED_LANGS.indexOf(lang)>=0)sel.value=lang;
  runnerGenerate();
}
function pickerUseCandidate(path, lang){
  var input=document.getElementById("picker-path-input");
  var sel=document.getElementById("picker-lang-select");
  if(input)input.value=path;
  if(sel&&SUPPORTED_LANGS.indexOf(lang)>=0)sel.value=lang;
  pickerAnalyzePath();
}

// ── Project Radar ────────────────────────────────────────────────
function radarEl(where){
  return document.getElementById(where==="picker"?"picker-project-radar":"runner-project-radar");
}
function projectInventory(path, where){
  if(!RUNNER.connected||!path)return Promise.resolve(null);
  return rapi("/api/project/inventory?root="+encodeURIComponent(path)).then(function(d){
    renderProjectRadar(d.data, where);
    return d.data;
  });
}
function radarStatusLabel(status){
  var map={
    root_project:"projectRadar.rootProject",
    single_candidate:"projectRadar.singleCandidate",
    multi_project:"projectRadar.multiProject",
    unsupported_only:"projectRadar.unsupportedOnly",
    empty:"projectRadar.empty",
    not_directory:"projectRadar.notDirectory",
    not_found:"projectRadar.notFound"
  };
  return tr(map[status]||"projectRadar.empty");
}
function radarStatusClass(status){
  if(status==="root_project"||status==="single_candidate")return"";
  if(status==="multi_project")return"warn";
  return"error";
}
function renderLangChips(langs, unsupported){
  return (langs||[]).map(function(l){
    return '<span class="project-radar-chip '+(unsupported?'unsupported':'')+'">'+esc(l)+'</span>';
  }).join("");
}
function renderProjectRadar(inv, where){
  var el=radarEl(where); if(!el||!inv)return;
  el.style.display="";
  var supported=inv.supportedLanguages||[];
  var unsupported=inv.unsupportedLanguages||[];
  var candidates=inv.candidates||[];
  var status=inv.status||"empty";
  var analyzeCurrent=(status==="root_project"&&inv.recommendedRoot);
  var html='<div class="project-radar-header"><span class="project-radar-title">⌁ '+esc(tr("projectRadar.title"))+'</span>'+
    '<span class="project-radar-status '+radarStatusClass(status)+'">'+esc(radarStatusLabel(status))+'</span></div>'+
    '<div class="project-radar-message">'+esc(inv.message||"")+'</div>'+
    '<div class="project-radar-langs">'+
      (supported.length?renderLangChips(supported,false):"")+
      (unsupported.length?renderLangChips(unsupported,true):"")+
    '</div>'+
    '<div class="text-muted text-sm">'+esc(tr("projectRadar.staticHint"))+'</div>';
  if(analyzeCurrent){
    html+='<div class="project-radar-actions"><button class="btn btn-sm btn-primary" onclick="radarUseRoot(&quot;'+where+'&quot;,&quot;'+escAttr(inv.recommendedRoot)+'&quot;,&quot;'+escAttr(inv.recommendedLanguage||"auto")+'&quot;)"><span class="btn-icon spark" aria-hidden="true"></span>'+esc(tr("projectRadar.analyzeHere"))+'</button></div>';
  }
  if(candidates.length){
    html+='<div class="project-radar-actions">'+candidates.map(function(c){
      var lang=c.analysisLanguage||candidateLang(c)||"auto";
      if(lang==="c/cpp")lang="cpp";
      var unsupportedOnly=!(c.supportedLanguages||[]).length;
      return '<button class="project-radar-candidate '+(unsupportedOnly?'unsupported':'')+'" '+(unsupportedOnly?'disabled':'onclick="radarUseRoot(&quot;'+where+'&quot;,&quot;'+escAttr(c.path)+'&quot;,&quot;'+escAttr(lang)+'&quot;)"')+'>'+
        '<span class="project-radar-path">'+esc(c.path)+'</span>'+
        '<span class="candidate-lang">'+esc((c.languages||[]).join(", "))+'</span>'+
        '<strong>'+esc(unsupportedOnly?tr("projectRadar.unsupported"):tr("projectRadar.chooseCandidate"))+'</strong>'+
      '</button>';
    }).join("")+'</div>';
  }
  el.innerHTML=html;
}
function radarUseRoot(where,path,lang){
  var input=document.getElementById(where==="picker"?"picker-path-input":"runner-root-input");
  var sel=document.getElementById(where==="picker"?"picker-lang-select":"runner-lang-select");
  if(input)input.value=path;
  if(sel&&SUPPORTED_LANGS.indexOf(lang)>=0)sel.value=lang;
  if(where==="picker")pickerAnalyzePath(); else runnerGenerate();
}
function analyzeAfterInventory(root, lang, where, run){
  if(!RUNNER.connected)return run(root,lang);
  return projectInventory(root,where).then(function(inv){
    if(!inv)return run(root,lang);
    if(lang==="auto"){
      if(inv.status==="multi_project"){
        var hint=where==="picker"?document.getElementById("picker-hint"):document.getElementById("runner-status");
        if(hint)hint.textContent=tr("workspace.scanning");
        return new Promise(function(resolve){
          workspaceScanInventory(root,function(scanErr){
            if(scanErr){
              if(hint)hint.textContent=tr("workspace.noSupported");
              resolve(null);
              return;
            }
            workspaceAnalyze(root,"recommended",null,function(runErr,ws){
              if(runErr){
                if(hint)hint.textContent="Workspace analysis failed: "+(runErr.message||runErr);
                resolve(null);
                return;
              }
              workspaceFocusRun(ws,{scroll:true});
              resolve(null);
            });
          });
        });
      }
      if(inv.status==="single_candidate"&&inv.recommendedRoot){
        return run(inv.recommendedRoot,inv.recommendedLanguage||"auto");
      }
      if(["multi_project","unsupported_only","empty","not_found","not_directory"].indexOf(inv.status)>=0){
        var hint=where==="picker"?document.getElementById("picker-hint"):document.getElementById("runner-status");
        if(hint)hint.textContent=radarStatusLabel(inv.status)+": "+(inv.message||"");
        return null;
      }
      if(inv.recommendedLanguage&&SUPPORTED_LANGS.indexOf(inv.recommendedLanguage)>=0){
        lang=inv.recommendedLanguage;
      }
    }
    return run(root,lang);
  }).catch(function(){
    return run(root,lang);
  });
}

// ── Workbench Project Folder Picker ─────────────────────────────
function runnerPickDirectory(){
  if(!RUNNER.connected){alert(tr("runner.startHint"));return;}
  var st=document.getElementById("runner-status");
  var input=document.getElementById("runner-root-input");
  var current=input?input.value.trim():"";
  if(st)st.textContent=tr("picker.folderPickerOpening");
  rapi("/api/fs/pick-directory",{method:"POST",body:{currentPath:current}}).then(function(d){
    var path=(d.data||{}).path||"";
    if(!path)throw new Error(tr("picker.browseUnavailable"));
    runnerSelectPath(path);
    if(st)st.textContent=tr("picker.selectedFolder");
  }).catch(function(e){
    if(st)st.textContent=tr("picker.folderPickerFallback");
    runnerBrowse(current || "/");
  });
}

function runnerBrowse(path){
  if(!RUNNER.connected){alert(tr("runner.startHint"));return;}
  var listEl=document.getElementById("runner-browse-list");
  if(!listEl)return;
  listEl.innerHTML='<div style="padding:8px;color:#9ca3af;">'+esc(tr("picker.browseLoading"))+'</div>';
  rapi("/api/fs/list?path="+encodeURIComponent(path)).then(function(d){
    var dd=d.data;
    if(!dd||!dd.entries){
      listEl.innerHTML='<div style="padding:8px;color:#dc2626;">'+esc(tr("picker.browseUnavailable"))+'</div>';
      return;
    }
    var input=document.getElementById("runner-root-input");
    if(input)input.value=dd.path;
    var parts=dd.path.split("/").filter(Boolean);
    var bc=parts.map(function(p,i){
      var bp="/"+parts.slice(0,i+1).join("/");
      return '<a href="#" onclick="runnerBrowse(&quot;'+escAttr(bp)+'&quot;);return false;" style="color:#2563eb;">'+esc(p)+'</a>';
    }).join(" / ");
    var dirs=dd.entries.filter(function(e){return e.isDir;});
    listEl.innerHTML='<div style="padding:2px 0;color:#6b7280;"><span class="btn-icon folder" aria-hidden="true"></span> / '+bc+'</div>'+
      '<div style="padding:4px 0;cursor:pointer;color:#059669;font-weight:600;" onclick="runnerSelectPath(&quot;'+escAttr(dd.path)+'&quot;)">✅ 选定此文件夹</div>'+
      dirs.map(function(e){
        return '<div class="browse-row" onclick="runnerBrowse(&quot;'+escAttr(e.path)+'&quot;)"><span class="btn-icon folder" aria-hidden="true"></span>'+esc(e.name)+'</div>';
      }).join("");
  }).catch(function(){
    listEl.innerHTML='<div style="padding:8px;color:#dc2626;">'+esc(tr("picker.browseUnavailable"))+'</div>';
  });
}

function runnerSelectPath(path){
  var input=document.getElementById("runner-root-input");
  var listEl=document.getElementById("runner-browse-list");
  if(input)input.value=path;
  if(listEl)listEl.innerHTML='<div style="padding:8px;color:#059669;">✅ '+esc(path)+' — '+esc(tr("picker.selectedFolder"))+'</div>';
  var st=document.getElementById("runner-status");
  if(st)st.textContent=tr("picker.selectedFolder");
  projectInventory(path,"runner");
}

function runnerLoadQuickRoots(){
  if(!RUNNER.connected)return;
  rapi("/api/fs/roots").then(function(d){
    var roots=d.data||[];
    var rootBtns=roots.map(function(r){
      return '<button class="btn btn-sm btn-secondary" onclick="runnerBrowse(&quot;'+escAttr(r.path)+'&quot;)">'+r.icon+' '+esc(r.label)+'</button>';
    }).join(" ");
    var el=document.getElementById("runner-quick-roots");
    if(el)el.innerHTML=rootBtns+' <button class="btn btn-sm btn-secondary" onclick="runnerBrowse(&quot;/&quot;)"><span class="btn-icon folder" aria-hidden="true"></span>/</button>';
  });
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
    '<input id="lib-search" class="search-input" style="max-width:180px;" placeholder="'+escAttr(tr("library.search"))+'" oninput="libraryFilter.q=this.value;runnerLoadLibrary()">'+
    '<select id="lib-lang" class="filter-select" onchange="libraryFilter.language=this.value;runnerLoadLibrary()"><option value="">'+esc(tr("library.allLang"))+'</option>'+SUPPORTED_LANGS.map(function(l){return '<option value="'+l+'">'+l+'</option>';}).join("")+'</select>'+
    '<select class="filter-select" onchange="var p=this.value.split(\":\");libraryFilter.sort=p[0];libraryFilter.order=p[1];runnerLoadLibrary()"><option value="createdAt:desc">'+esc(tr("library.newest"))+'</option><option value="createdAt:asc">'+esc(tr("library.oldest"))+'</option><option value="symbolCount:desc">'+esc(tr("library.mostSymbols"))+'</option><option value="sourceFileCount:desc">'+esc(tr("library.mostFiles"))+'</option></select>'+
    '<button class="btn btn-sm btn-secondary" onclick="runnerLoadLibrary()">'+esc(tr("library.refresh"))+'</button></div>';
  if(s.length===0){el.innerHTML=html+'<span class="text-muted text-sm">'+esc(tr("library.noSnapshots"))+'</span>';return;}
  el.innerHTML=html+'<div style="display:flex;gap:6px;flex-wrap:wrap;">'+s.map(function(e){
    var sm=e.summary||{},sc=e.secondary||{};
    return '<div class="snap-card" style="padding:8px 10px;background:#f8fafc;border:1px solid #e5e7eb;border-radius:4px;font-size:0.82em;min-width:220px;">'+
      '<div style="display:flex;justify-content:space-between;align-items:center;">'+
      '<strong title="'+esc(e.id)+'">'+(e.profileName?esc(e.profileName)+" · ":"")+esc(e.createdAt||"").slice(0,16)+'</strong>'+
      '<span class="badge badge-lang">'+esc(e.language||"?")+'</span></div>'+
      '<div style="color:#6b7280;">'+esc(e.rootLabel||"")+' &middot; '+sm.symbolCount+' '+esc(tr("library.syms"))+', '+sm.sourceFileCount+' '+esc(tr("library.files"))+'</div>'+
      '<div style="margin-top:4px;display:flex;gap:4px;flex-wrap:wrap;">'+
      '<button class="btn btn-sm btn-primary" onclick="runnerLoadSnap(&quot;'+escAttr(e.id)+'&quot;)">'+esc(tr("library.load"))+'</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerCompareSnap(&quot;'+escAttr(e.id)+'&quot;)">'+esc(tr("library.diff"))+'</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerAddTimeline(&quot;'+escAttr(e.id)+'&quot;)">'+esc(tr("library.timeline"))+'</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="runnerDownloadSnap(&quot;'+escAttr(e.id)+'&quot;)">'+esc(tr("library.download"))+'</button>'+
      '<button class="btn btn-sm btn-secondary" onclick="if(confirm(&quot;'+escAttr(tr("library.deleteConfirm"))+'&quot;))runnerDeleteSnap(&quot;'+escAttr(e.id)+'&quot;)" style="color:#dc2626;">×</button></div></div>';
  }).join("")+'</div>';
}
function runnerLoadSnap(sid, opts){
  opts=opts||{};
  if(!RUNNER.connected)return;
  return rapi("/api/snapshot/"+sid).then(function(d){
    openWorkbenchSnapshot(d.data,sid,opts.tab?opts:{tab:"dashboard"});
  }).catch(function(e){if(!opts.silent)alert(e.message); throw e;});
}
function runnerCompareSnap(sid){if(!RUNNER.connected)return;
  rapi("/api/snapshot/"+sid).then(function(d){diffSnapshot=d.data;
    document.getElementById("diff-compare-name").textContent=tr("library.vs")+" "+sid;
    showEl("diff-clear-btn",true);computeAndRenderDiff();show("diff");}).catch(function(e){alert(e.message);});}
function runnerAddTimeline(sid){if(!RUNNER.connected||typeof CTL==="undefined")return;
  rapi("/api/snapshot/"+sid).then(function(d){CTL.timelineSnapshots=CTL.timelineSnapshots||[];
    CTL.timelineSnapshots.push({name:sid+".json",data:d.data});
    CTL.timelineSnapshots.sort(function(a,b){return(a.data.generatedAt||"").localeCompare(b.data.generatedAt||"");});
    document.getElementById("timeline-snapshot-count").textContent=CTL.timelineSnapshots.length+" "+tr("library.snaps");
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
    el.innerHTML='<h3>'+esc(tr("guided.title"))+'</h3><p class="text-muted">'+esc(tr("guided.select"))+'</p>'+
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
    '<button class="btn btn-sm btn-primary" onclick="guidedReport()">'+esc(tr("guided.report"))+'</button>'+
    '<button class="btn btn-sm btn-secondary" onclick="guidedReset()">'+esc(tr("guided.reset"))+'</button>'+
    '<button class="btn btn-sm btn-secondary" onclick="RUNNER.guidedScenario=null;guidedRender();">'+esc(tr("common.back"))+'</button></div>'+
    '<div style="font-size:0.88em;">'+sc.steps.map(function(s,i){
      return '<label style="display:flex;align-items:flex-start;gap:6px;padding:4px 0;cursor:pointer;"><input type="checkbox" '+
        (checks[i]?'checked':'')+' onchange="guidedToggle(&quot;'+sc.id+'&quot;,'+i+',this.checked)">'+esc(s)+'</label>';
    }).join("")+'</div>'+
    '<div class="caution-box" style="margin-top:8px;"><strong>'+esc(tr("guided.humanAid"))+'</strong></div>';
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
    listEl.innerHTML='<div style="padding:2px 0;color:#6b7280;"><span class="btn-icon folder" aria-hidden="true"></span> / '+bc+'</div>'+
      '<div style="padding:4px 0;cursor:pointer;color:#059669;font-weight:600;" onclick="pickerSelect(&quot;'+escAttr(dd.path)+'&quot;)">✅ 选定此文件夹</div>'+
      dirs.map(function(e){
        return '<div class="browse-row" onclick="pickerBrowse(&quot;'+escAttr(e.path)+'&quot;)"><span class="btn-icon folder" aria-hidden="true"></span>'+esc(e.name)+'</div>';
      }).join("");
    // 更新快速入口
    document.getElementById("picker-quick-roots").innerHTML=
      '<button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;/&quot;)"><span class="btn-icon folder" aria-hidden="true"></span>/</button> ';
  });
}

function pickerSelect(path){
  document.getElementById("picker-path-input").value=path;
  document.getElementById("picker-browse-list").innerHTML='<div style="padding:8px;color:#059669;">✅ '+esc(path)+' — '+esc(CTL_I18N.t("picker.selectedFolder"))+'</div>';
  projectInventory(path,"picker");
}

// Runner 连接时加载快速入口
function pickerLoadQuickRoots(){
  if(!RUNNER.connected)return;
  rapi("/api/fs/roots").then(function(d){
    var roots=d.data||[];
    var rootBtns=roots.map(function(r){
      return '<button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;'+escAttr(r.path)+'&quot;)">'+r.icon+' '+esc(r.label)+'</button>';
    }).join(" ");
    document.getElementById("picker-quick-roots").innerHTML=rootBtns+' <button class="btn btn-sm btn-secondary" onclick="pickerBrowse(&quot;/&quot;)"><span class="btn-icon folder" aria-hidden="true"></span>/</button>';
  });
}

function pickerAnalyzePath(){
  var root=document.getElementById("picker-path-input").value.trim();
  if(!root){alert(tr("error.missingRoot")); return;}
  var lang=document.getElementById("picker-lang-select").value;
  document.getElementById("picker-hint").textContent=tr("gen.generating");
  return analyzeAfterInventory(root,lang,"picker",function(targetRoot,targetLang){
    document.getElementById("picker-hint").textContent=tr("gen.generating");
    return rapi("/api/quick-analyze",{method:"POST",body:{root:targetRoot,language:targetLang}}).then(function(d){
      openWorkbenchSnapshot(d.data.snapshot,d.data.snapshotId,{tab:"dashboard"});
      pickerRefresh();
    }).catch(function(e){
      var msg=e.message+(e.hint?" — "+e.hint:"");
      document.getElementById("picker-hint").textContent=msg;
      showGenerationError(e, "picker");
      // 如果是多项目错误，尝试转到工作区视图
      if (typeof showWorkspaceOverview === "function" && e.hint && e.hint.indexOf("候选子项目") !== -1) {
        showWorkspaceOverview(root);
      }
    });
  }).then(function(result) {
    // analyzeAfterInventory may return null for multi_project
    if (result === null && typeof showWorkspaceOverview === "function") {
      showWorkspaceOverview(root);
    }
  });
}
function pickerAnalyzeProfile(pid){
  document.getElementById("picker-hint").textContent=CTL_I18N.t("gen.generating");
  rapi("/api/profile/"+pid).then(function(d){
    var p=d.data; document.getElementById("runner-root-input").value=p.root;
    document.getElementById("runner-lang-select").value=p.language;
    return rapi("/api/quick-analyze",{method:"POST",body:{root:p.root,language:p.language}});
  }).then(function(d){
    openWorkbenchSnapshot(d.data.snapshot,d.data.snapshotId,{tab:"dashboard"});
    pickerRefresh();
  }).catch(function(e){alert(e.message); document.getElementById("picker-hint").textContent=CTL_I18N.t("picker.openProject");});
}

// ── Workspace API ──────────────────────────────────────────────────
var WORKSPACE = window.WORKSPACE || {};
WORKSPACE.state = { inventory: null, selectedForAnalysis: [], runs: [], currentRunId: null, currentRun: null, insights: null, graph: null, impact: null };

function workspaceScanInventory(root, cb) {
  if (!RUNNER.connected) { alert(tr("runner.notConnected")); return; }
  var hint = document.getElementById("picker-hint");
  if (hint) hint.textContent = tr("workspace.scanning");
  rapi("/api/workspace/inventory?root=" + encodeURIComponent(root)).then(function(d) {
    WORKSPACE.state.inventory = d.data;
    if (cb) cb(null, d.data);
    if (hint) hint.textContent = tr("workspace.scanResult", {
      supported: d.data.summary.supportedProjectCount,
      unsupported: d.data.summary.unsupportedModuleCount
    });
  }).catch(function(e) {
    if (cb) cb(e);
    if (hint) hint.textContent = tr("workspace.noSupported");
    console.error("workspace scan error:", e);
  });
}

function workspaceAnalyze(root, mode, projectIds, cb) {
  if (!RUNNER.connected) { alert(tr("runner.notConnected")); return; }
  var body = { root: root, mode: mode, redactRoot: true };
  if (projectIds && projectIds.length) body.projectIds = projectIds;
  rapi("/api/workspace/analyze", { method: "POST", body: body }).then(function(d) {
    WORKSPACE.state.currentRunId = d.data.workspaceId;
    WORKSPACE.state.currentRun = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace analyze error:", e);
    if (cb) cb(e);
  });
}

function workspaceLoadRuns(cb) {
  if (!RUNNER.connected) return;
  rapi("/api/workspace/runs").then(function(d) {
    WORKSPACE.state.runs = d.data || [];
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace runs error:", e);
    if (cb) cb(e);
  });
}

function workspaceGetRun(wid, cb) {
  if (!RUNNER.connected) return;
  rapi("/api/workspace/run/" + wid).then(function(d) {
    WORKSPACE.state.currentRun = d.data;
    WORKSPACE.state.currentRunId = d.data.workspaceId;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace run get error:", e);
    if (cb) cb(e);
  });
}

// ── Workspace Actions ─────────────────────────────────────────────

function workspaceAnalyzeRecommended() {
  var inv = WORKSPACE.state.inventory;
  if (!inv) return;
  var root = inv.root || "";
  workspaceAnalyze(root, "recommended", null, function(err, ws) {
    if (err) { alert("Workspace analysis failed: " + (err.message || err)); return; }
    workspaceFocusRun(ws, {scroll: true});
  });
}

function workspaceAnalyzeSelected() {
  var inv = WORKSPACE.state.inventory;
  if (!inv) return;
  var root = inv.root || "";
  var cbs = document.querySelectorAll(".ws-checkbox:checked");
  var ids = [];
  cbs.forEach(function(cb) { ids.push(cb.getAttribute("data-path") || cb.value); });
  if (ids.length === 0) { alert(tr("workspace.selectProjects")); return; }
  workspaceAnalyze(root, "selected", ids, function(err, ws) {
    if (err) { alert("Workspace analysis failed: " + (err.message || err)); return; }
    workspaceFocusRun(ws, {scroll: true});
  });
}

function workspaceFocusRun(ws, opts) {
  opts = opts || {};
  if (!ws) return;
  WORKSPACE.state.currentRun = ws;
  WORKSPACE.state.currentRunId = ws.workspaceId || WORKSPACE.state.currentRunId;
  try { localStorage.setItem("codelattice.workspace.lastRunId", WORKSPACE.state.currentRunId || ""); } catch(e) {}
  showEl("loaded-content", true);
  showEl("welcome-view", false);
  showEl("error-view", false);
  show("workspace");
  if (WORKSPACE.state.inventory && typeof renderWorkspace === "function") {
    renderWorkspace(WORKSPACE.state.inventory);
  }
  workspaceLoadRuns(function(e, runs) {
    if (!e && runs && typeof renderWorkspaceRuns === "function") renderWorkspaceRuns(runs);
    workspaceLoadInsights(WORKSPACE.state.currentRunId, function(e2, ins) {
      if (typeof renderWorkspaceInsights === "function") renderWorkspaceInsights(e2 ? null : ins);
      if (opts.scroll !== false) {
        var v = document.getElementById("view-workspace");
        if (v && v.scrollIntoView) v.scrollIntoView({behavior: "smooth", block: "start"});
      }
    });
  });
}

function workspaceLoadProjectSnapshot(projId, projPath) {
  if (!RUNNER.connected) { alert(tr("runner.notConnected")); return; }
  var hint = document.getElementById("picker-hint");
  if (hint) hint.textContent = tr("gen.generating");
  rapi("/api/quick-analyze", { method: "POST", body: { root: projPath, language: "auto" } }).then(function(d) {
    openWorkbenchSnapshot(d.data.snapshot, d.data.snapshotId, { tab: "dashboard" });
    pickerRefresh();
  }).catch(function(e) {
    var msg = e.message + (e.hint ? " — " + e.hint : "");
    if (hint) hint.textContent = msg;
  });
}

// ── Workspace Insights ────────────────────────────────────────────

function workspaceLoadInsights(runId, cb) {
  if (!RUNNER.connected) return;
  rapi("/api/workspace/insights?runId=" + encodeURIComponent(runId)).then(function(d) {
    WORKSPACE.state.insights = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace insights error:", e);
    if (cb) cb(e);
  });
}

// ── Workspace Cross-Project Graph ─────────────────────────────────

function workspaceLoadGraph(runId, cb) {
  if (!RUNNER.connected) return;
  rapi("/api/workspace/graph?runId=" + encodeURIComponent(runId)).then(function(d) {
    WORKSPACE.state.graph = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace graph error:", e);
    if (cb) cb(e);
  });
}

function workspaceCopyGraphAiSummary() {
  var graph = WORKSPACE.state.graph;
  if (!graph) { alert(tr("workspace.graphNotLoaded")); return; }
  var text = buildWorkspaceGraphAiSummary(graph);
  var status = document.getElementById("workspace-graph-status");
  function done(ok) {
    if (status) status.textContent = ok ? tr("workspace.aiSummaryCopied") : tr("workspace.aiSummarySelect");
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(function(){ done(true); }, function(){ fallbackCopyWorkspaceSummary(text, done); });
  } else {
    fallbackCopyWorkspaceSummary(text, done);
  }
}

function buildWorkspaceGraphAiSummary(graph) {
  var zh = window.CTL_I18N && CTL_I18N.lang === "zh";
  var sm = graph.summary || {};
  var lines = [];
  if (zh) {
    lines.push("CodeLattice 工作区跨项目关系图摘要（给 AI 使用）");
    lines.push("注意：这是静态启发式结果，不证明运行时依赖关系。");
    lines.push("");
    lines.push("节点：" + (sm.nodeCount || 0) + "，边：" + (sm.edgeCount || 0));
    lines.push("跨项目边：" + (sm.crossProjectEdgeCount || 0) + "，不支持模块边界：" + (sm.unsupportedBoundaryCount || 0));
    lines.push("项目：" + (sm.projectCount || 0) + "，不支持模块：" + (sm.unsupportedModuleCount || 0));
  } else {
    lines.push("CodeLattice Workspace Cross-Project Graph Summary (for AI)");
    lines.push("Caution: static heuristic output only; does not prove runtime dependencies.");
    lines.push("");
    lines.push("Nodes: " + (sm.nodeCount || 0) + ", Edges: " + (sm.edgeCount || 0));
    lines.push("Cross-project edges: " + (sm.crossProjectEdgeCount || 0) + ", Unsupported boundaries: " + (sm.unsupportedBoundaryCount || 0));
    lines.push("Projects: " + (sm.projectCount || 0) + ", Unsupported modules: " + (sm.unsupportedModuleCount || 0));
  }
  // Top connected projects
  var topProjects = graph.topConnectedProjects || [];
  if (topProjects.length > 0) {
    lines.push("");
    lines.push(zh ? "高连接度项目：" : "Top connected projects:");
    topProjects.forEach(function(p) {
      lines.push("- " + (p.label || "?") + " (" + (p.connections || 0) + " " + (zh ? "连接" : "connections") + ")");
    });
  }
  // Bridge scripts
  var bridgeScripts = graph.bridgeScripts || [];
  if (bridgeScripts.length > 0) {
    lines.push("");
    lines.push(zh ? "桥接脚本（被多处引用）：" : "Bridge scripts (referenced from multiple places):");
    bridgeScripts.forEach(function(b) {
      lines.push("- " + (b.label || "?") + " (" + (b.refCount || 0) + " " + (zh ? "引用" : "refs") + ")");
    });
  }
  // Bridge configs
  var bridgeConfigs = graph.bridgeConfigs || [];
  if (bridgeConfigs.length > 0) {
    lines.push("");
    lines.push(zh ? "桥接配置（被多处引用）：" : "Bridge configs:");
    bridgeConfigs.forEach(function(b) {
      lines.push("- " + (b.label || "?") + " (" + (b.refCount || 0) + " " + (zh ? "引用" : "refs") + ")");
    });
  }
  // Edge list (first 30)
  var edges = (graph.edges || []).filter(function(e) { return e.kind !== "contains"; }).slice(0, 30);
  if (edges.length > 0) {
    lines.push("");
    lines.push(zh ? "跨项目关系（前 30 条）：" : "Cross-project relationships (first 30):");
    var nodesById = {};
    (graph.nodes || []).forEach(function(n) { nodesById[n.id] = n; });
    edges.forEach(function(e) {
      var srcLabel = (nodesById[e.source] || {}).label || e.source;
      var tgtLabel = (nodesById[e.target] || {}).label || e.target;
      lines.push("- " + srcLabel + " → " + tgtLabel + " [" + e.kind + "] conf=" + (e.confidence || 0) + " " + (e.reason || ""));
    });
  }
  lines.push("");
  lines.push(zh ? "建议：基于这些线索规划跨项目审查；不要将边视为事实依赖。" : "Recommendation: use these as investigation leads; do not treat edges as proven dependencies.");
  return lines.join("\n");
}

function workspaceOpenInsightSnapshot(snapshotId, tab) {
  if (!snapshotId) {
    alert(tr("workspace.noSnapshot"));
    return;
  }
  openWorkbenchSnapshot(null, snapshotId, {tab: tab || "dashboard"});
}

// ── Workspace Cross-Project Graph ─────────────────────────────────

function workspaceLoadGraph(runId, cb) {
  if (!RUNNER.connected) return;
  rapi("/api/workspace/graph?runId=" + encodeURIComponent(runId)).then(function(d) {
    WORKSPACE.state.graph = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace graph error:", e);
    if (cb) cb(e);
  });
}

function workspaceCopyGraphAiSummary() {
  var graph = WORKSPACE.state.graph;
  var ins = WORKSPACE.state.insights;
  var zh = window.CTL_I18N && CTL_I18N.lang === "zh";
  if (!graph) return;
  var lines = [];
  if (zh) {
    lines.push("CodeLattice 工作区跨项目关系图摘要（给 AI 使用）");
    lines.push("注意：这是静态启发式结果，不是编译或运行时依赖证明。");
    lines.push("");
    lines.push("节点数：" + (graph.summary.nodeCount || 0) + "，边数：" + (graph.summary.edgeCount || 0));
    lines.push("跨项目关系边：" + (graph.summary.crossProjectEdgeCount || 0) + "，不支持模块边界：" + (graph.summary.unsupportedBoundaryCount || 0));
  } else {
    lines.push("CodeLattice Workspace Cross-Project Graph Summary (for AI)");
    lines.push("Note: static heuristic only, not compile/runtime dependency proof.");
    lines.push("");
    lines.push("Nodes: " + (graph.summary.nodeCount || 0) + ", Edges: " + (graph.summary.edgeCount || 0));
    lines.push("Cross-project edges: " + (graph.summary.crossProjectEdgeCount || 0) + ", Unsupported boundaries: " + (graph.summary.unsupportedBoundaryCount || 0));
  }
  if (graph.topConnectedProjects && graph.topConnectedProjects.length > 0) {
    lines.push("");
    lines.push(zh ? "高连接度项目：" : "Top connected projects:");
    graph.topConnectedProjects.forEach(function(p) {
      lines.push("  - " + p.label + " (" + p.connections + " " + (zh ? "连接" : "connections") + ")");
    });
  }
  if (graph.bridgeScripts && graph.bridgeScripts.length > 0) {
    lines.push("");
    lines.push(zh ? "桥接脚本：" : "Bridge scripts:");
    graph.bridgeScripts.forEach(function(s) { lines.push("  - " + (s.label || s.id)); });
  }
  if (graph.bridgeConfigs && graph.bridgeConfigs.length > 0) {
    lines.push("");
    lines.push(zh ? "桥接配置：" : "Bridge configs:");
    graph.bridgeConfigs.forEach(function(c) { lines.push("  - " + (c.label || c.id)); });
  }
  var text = lines.join("\n");
  try { navigator.clipboard.writeText(text); } catch(e) {}
  return text;
}

function buildWorkspaceAiSummary() {
  var ins = WORKSPACE.state.insights;
  var run = WORKSPACE.state.currentRun || {};
  var inv = WORKSPACE.state.inventory || {};
  var zh = window.CTL_I18N && CTL_I18N.lang === "zh";
  if (!ins) return zh ? "尚未生成工作区洞察。请先分析推荐项目。" : "No workspace insights generated yet. Analyze recommended projects first.";
  var sm = ins.summary || {};
  var cp = ins.crossProjectSignals || {};
  var lines = [];
  if (zh) {
    lines.push("CodeLattice 工作区静态分析摘要（给 AI 使用）");
    lines.push("注意：这是静态启发式结果，不是编译、运行时、测试覆盖或删除安全证明。");
    lines.push("");
    lines.push("工作区：" + (inv.root || run.root || "unknown"));
    lines.push("健康分：" + (sm.overallHealthScore == null ? "unknown" : sm.overallHealthScore + "/100") + "，风险等级：" + (sm.overallRiskLevel || "unknown"));
    lines.push("项目：" + (sm.succeededProjectCount || 0) + " 成功 / " + (sm.failedProjectCount || 0) + " 失败 / " + (sm.projectCount || 0) + " 总计；暂不支持模块：" + (sm.unsupportedModuleCount || 0));
    lines.push("规模：源文件 " + (sm.totalSourceFiles || 0) + "，符号 " + (sm.totalSymbols || 0) + "，边 " + (sm.totalEdges || 0));
    appendAiList(lines, "先读这些", ins.readFirst);
    appendAiList(lines, "优先审查", ins.reviewFirst);
    appendAiList(lines, "优先清理", ins.cleanupFirst);
    appendUnsupported(lines, "暂不支持语言集群", cp.unsupportedLanguageClusters);
    // Cross-project graph summary
    var gps = ins.crossProjectGraphSummary || {};
    if (gps.available) {
      lines.push("");
      lines.push("跨项目关系图：节点 " + (gps.nodeCount || 0) + "，边 " + (gps.edgeCount || 0) + "，跨项目边 " + (gps.crossProjectEdgeCount || 0) + "，不支持模块边界 " + (gps.unsupportedBoundaryCount || 0));
      if (gps.topConnectedProjects && gps.topConnectedProjects.length > 0) {
        lines.push("高连接度项目：" + gps.topConnectedProjects.map(function(p){ return p.label + "(" + p.connections + ")"; }).join(", "));
      }
    }
    // Impact hints
    var impH = ins.crossProjectImpactHints || {};
    if (impH.available) {
      lines.push("");
      lines.push("跨项目影响分析线索：");
      var sug = impH.suggestedImpactTargets || [];
      if (sug.length) { lines.push("建议分析目标：" + sug.map(function(s){ return s.label; }).join(", ")); }
      var hfp = impH.highFanoutProjects || [];
      if (hfp.length) { lines.push("高连接度：" + hfp.map(function(p){ return p.label + "(" + p.total + ")"; }).join(", ")); }
    }
    lines.push("");
    lines.push("建议：请基于这些线索规划阅读/审查顺序；不要把候选项当作事实结论。");
  } else {
    lines.push("CodeLattice Workspace Static Analysis Summary (for AI)");
    lines.push("Caution: static heuristic output only; not compile, runtime, coverage, or deletion-safety proof.");
    lines.push("");
    lines.push("Workspace: " + (inv.root || run.root || "unknown"));
    lines.push("Health: " + (sm.overallHealthScore == null ? "unknown" : sm.overallHealthScore + "/100") + ", risk: " + (sm.overallRiskLevel || "unknown"));
    lines.push("Projects: " + (sm.succeededProjectCount || 0) + " succeeded / " + (sm.failedProjectCount || 0) + " failed / " + (sm.projectCount || 0) + " total; unsupported modules: " + (sm.unsupportedModuleCount || 0));
    lines.push("Scale: " + (sm.totalSourceFiles || 0) + " files, " + (sm.totalSymbols || 0) + " symbols, " + (sm.totalEdges || 0) + " edges");
    appendAiList(lines, "Read first", ins.readFirst);
    appendAiList(lines, "Review first", ins.reviewFirst);
    appendAiList(lines, "Cleanup first", ins.cleanupFirst);
    appendUnsupported(lines, "Unsupported language clusters", cp.unsupportedLanguageClusters);
    var gps = ins.crossProjectGraphSummary || {};
    if (gps.available) {
      lines.push("");
      lines.push("Cross-project graph: nodes " + (gps.nodeCount || 0) + ", edges " + (gps.edgeCount || 0) + ", cross-project " + (gps.crossProjectEdgeCount || 0) + ", unsupported boundaries " + (gps.unsupportedBoundaryCount || 0));
      if (gps.topConnectedProjects && gps.topConnectedProjects.length > 0) {
        lines.push("Top connected: " + gps.topConnectedProjects.map(function(p){ return p.label + "(" + p.connections + ")"; }).join(", "));
      }
    }
    var impH = ins.crossProjectImpactHints || {};
    if (impH.available) {
      lines.push("");
      lines.push("Cross-project impact hints:");
      var sug = impH.suggestedImpactTargets || [];
      if (sug.length) { lines.push("Suggested targets: " + sug.map(function(s){ return s.label; }).join(", ")); }
      var hfp = impH.highFanoutProjects || [];
      if (hfp.length) { lines.push("High fanout: " + hfp.map(function(p){ return p.label + "(" + p.total + ")"; }).join(", ")); }
    }
    lines.push("");
    lines.push("Recommendation: use these as investigation leads and plan the next review steps manually.");
  }
  return lines.join("\n");
}

function appendAiList(lines, title, items) {
  items = items || [];
  if (!items.length) return;
  lines.push("");
  lines.push(title + ":");
  items.slice(0, 8).forEach(function(item) {
    lines.push("- [" + (item.priority || "P?") + "] " + (item.projectId || item.name || "?") + " — " + (item.reason || ""));
  });
}

function appendUnsupported(lines, title, clusters) {
  clusters = clusters || [];
  if (!clusters.length) return;
  lines.push("");
  lines.push(title + ":");
  clusters.slice(0, 8).forEach(function(c) {
    lines.push("- " + (c.language || "?") + ": " + (c.count || 0));
  });
}

function copyWorkspaceAiSummary() {
  var text = buildWorkspaceAiSummary();
  var status = document.getElementById("workspace-ai-summary-status");
  function done(ok) {
    if (status) status.textContent = ok ? tr("workspace.aiSummaryCopied") : tr("workspace.aiSummarySelect");
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(function(){ done(true); }, function(){ fallbackCopyWorkspaceSummary(text, done); });
  } else {
    fallbackCopyWorkspaceSummary(text, done);
  }
}

function fallbackCopyWorkspaceSummary(text, done) {
  try {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.setAttribute("readonly", "readonly");
    ta.style.position = "fixed";
    ta.style.left = "-9999px";
    document.body.appendChild(ta);
    ta.select();
    var ok = document.execCommand("copy");
    document.body.removeChild(ta);
    done(!!ok);
  } catch(e) {
    done(false);
  }
}

// ── Workspace Cross-Project Impact ──────────────────────────────────

function workspaceRunImpact(runId, projectId, direction, cb) {
  if (!RUNNER.connected) return;
  var body = { workspaceRunId: runId, target: { projectId: projectId }, direction: direction || "both", maxDepth: 3 };
  rapi("/api/workspace/impact", { method: "POST", body: body }).then(function(d) {
    WORKSPACE.state.impact = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace impact error:", e);
    if (cb) cb(e);
  });
}

function workspaceRunImpactByPath(runId, path, direction, cb) {
  if (!RUNNER.connected) return;
  var body = { workspaceRunId: runId, target: { path: path }, direction: direction || "both", maxDepth: 3 };
  rapi("/api/workspace/impact", { method: "POST", body: body }).then(function(d) {
    WORKSPACE.state.impact = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    console.error("workspace impact error:", e);
    if (cb) cb(e);
  });
}

function buildWorkspaceImpactAiSummary(impact) {
  if (!impact) return "";
  var zh = window.CTL_I18N && CTL_I18N.lang === "zh";
  var sm = impact.summary || {};
  var tgt = impact.target || {};
  var lines = [];
  if (zh) {
    lines.push("CodeLattice 跨项目影响分析摘要（给 AI 使用）");
    lines.push("注意：这是静态启发式结果，不证明运行时破坏。");
    lines.push("");
    lines.push("目标：" + (tgt.label || tgt.resolvedNodeId || "?") + "（" + (tgt.resolvedKind || "?") + "）");
    lines.push("方向：" + (sm.direction || "?") + "，风险等级：" + (sm.riskLevel || "?") + "，置信度：" + (sm.confidence || "?"));
    lines.push("受影响项目：" + (sm.affectedProjectCount || 0) + "，配置：" + (sm.affectedConfigCount || 0) + "，脚本：" + (sm.affectedScriptCount || 0) + "，工作流：" + (sm.affectedWorkflowCount || 0));
    var projs = impact.affectedProjects || [];
    if (projs.length) {
      lines.push("");
      lines.push("受影响项目列表：");
      projs.slice(0, 8).forEach(function(p) { lines.push("- " + (p.label||"?") + " (距离:" + p.distance + ", 置信度:" + p.confidence + ")"); });
    }
    var reasons = impact.riskReasons || [];
    if (reasons.length) {
      lines.push("");
      lines.push("风险原因：");
      reasons.forEach(function(r) { lines.push("- " + r); });
    }
    var checklist = impact.reviewChecklist || [];
    if (checklist.length) {
      lines.push("");
      lines.push("审查清单：");
      checklist.forEach(function(c) { lines.push("- [ ] " + c); });
    }
  } else {
    lines.push("CodeLattice Cross-Project Impact Analysis Summary (for AI)");
    lines.push("Caution: static heuristic output only; does not prove runtime breakage.");
    lines.push("");
    lines.push("Target: " + (tgt.label || tgt.resolvedNodeId || "?") + " (" + (tgt.resolvedKind || "?") + ")");
    lines.push("Direction: " + (sm.direction || "?") + ", Risk: " + (sm.riskLevel || "?") + ", Confidence: " + (sm.confidence || "?"));
    lines.push("Affected projects: " + (sm.affectedProjectCount || 0) + ", configs: " + (sm.affectedConfigCount || 0) + ", scripts: " + (sm.affectedScriptCount || 0) + ", workflows: " + (sm.affectedWorkflowCount || 0));
    var projs = impact.affectedProjects || [];
    if (projs.length) {
      lines.push("");
      lines.push("Affected Projects:");
      projs.slice(0, 8).forEach(function(p) { lines.push("- " + (p.label||"?") + " (distance:" + p.distance + ", confidence:" + p.confidence + ")"); });
    }
    var reasons = impact.riskReasons || [];
    if (reasons.length) {
      lines.push("");
      lines.push("Risk Reasons:");
      reasons.forEach(function(r) { lines.push("- " + r); });
    }
    var checklist = impact.reviewChecklist || [];
    if (checklist.length) {
      lines.push("");
      lines.push("Review Checklist:");
      checklist.forEach(function(c) { lines.push("- [ ] " + c); });
    }
  }
  return lines.join("\n");
}

function workspaceCopyImpactAiSummary() {
  var impact = WORKSPACE.state.impact;
  if (!impact) { alert(tr("workspace.impactNotLoaded")); return; }
  var text = buildWorkspaceImpactAiSummary(impact);
  var status = document.getElementById("workspace-impact-status");
  function done(ok) {
    if (status) status.textContent = ok ? tr("workspace.aiSummaryCopied") : tr("workspace.aiSummarySelect");
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(function() { done(true); }).catch(function() { fallbackCopyWorkspaceSummary(text, done); });
  } else {
    fallbackCopyWorkspaceSummary(text, done);
  }
}

// ── Cross-Project Impact API ────────────────────────────────────

function workspaceLoadImpact(runId, target, direction, cb) {
  if (!runId) { if (cb) cb("no runId"); return; }
  var body = { workspaceRunId: runId, target: target, direction: direction || "both", maxDepth: 3, includeUnsupported: true };
  rapi("/api/workspace/impact", { method: "POST", body: body }).then(function(d) {
    WORKSPACE.state.impact = d.data;
    if (cb) cb(null, d.data);
  }).catch(function(e) {
    if (cb) cb(e);
    console.error("workspace impact error:", e);
  });
}

function buildWorkspaceImpactAiSummary(impact) {
  if (!impact) return "";
  var zh = window.CTL_I18N && CTL_I18N.lang === "zh";
  var t = impact.target || {};
  var sm = impact.summary || {};
  var lines = [];
  if (zh) {
    lines.push("CodeLattice 跨项目影响分析摘要（给 AI 使用）");
    lines.push("注意：这是静态启发式结果，不证明运行时影响。");
    lines.push("");
    lines.push("目标：" + (t.label || "unknown") + " (" + (t.resolvedKind || "?") + ")");
    lines.push("方向：" + (sm.direction || "?") + "，风险等级：" + (sm.riskLevel || "unknown"));
    lines.push("受影响项目：" + (sm.affectedProjectCount || 0) + "，配置：" + (sm.affectedConfigCount || 0) + "，脚本：" + (sm.affectedScriptCount || 0) + "，工作流：" + (sm.affectedWorkflowCount || 0));
    lines.push("不支持边界：" + (sm.unsupportedBoundaryCount || 0));
  } else {
    lines.push("CodeLattice Cross-Project Impact Summary (for AI)");
    lines.push("Caution: static heuristic output only; does not prove runtime impact.");
    lines.push("");
    lines.push("Target: " + (t.label || "unknown") + " (" + (t.resolvedKind || "?") + ")");
    lines.push("Direction: " + (sm.direction || "?") + ", risk: " + (sm.riskLevel || "unknown"));
    lines.push("Affected projects: " + (sm.affectedProjectCount || 0) + ", configs: " + (sm.affectedConfigCount || 0) + ", scripts: " + (sm.affectedScriptCount || 0) + ", workflows: " + (sm.affectedWorkflowCount || 0));
    lines.push("Unsupported boundaries: " + (sm.unsupportedBoundaryCount || 0));
  }
  var ap = impact.affectedProjects || [];
  if (ap.length > 0) {
    lines.push("");
    lines.push(zh ? "受影响项目：" : "Affected projects:");
    ap.slice(0, 8).forEach(function(p) {
      lines.push("- " + (p.label || "?") + " (distance: " + p.distance + ", conf: " + (p.confidence || 0) + ")");
    });
  }
  var rr = impact.riskReasons || [];
  if (rr.length > 0) {
    lines.push("");
    lines.push(zh ? "风险原因：" : "Risk reasons:");
    rr.forEach(function(r) { lines.push("- " + r); });
  }
  var rc = impact.reviewChecklist || [];
  if (rc.length > 0) {
    lines.push("");
    lines.push(zh ? "建议审查清单：" : "Review checklist:");
    rc.forEach(function(r) { lines.push("- [ ] " + r); });
  }
  return lines.join("\n");
}

function workspaceCopyImpactAiSummary() {
  var impact = WORKSPACE.state.impact;
  if (!impact) { alert(tr("workspace.impactNotLoaded")); return; }
  var text = buildWorkspaceImpactAiSummary(impact);
  var status = document.getElementById("workspace-impact-status");
  function done(ok) {
    if (status) status.textContent = ok ? tr("workspace.aiSummaryCopied") : tr("workspace.aiSummarySelect");
  }
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(function() { done(true); }).catch(function() { fallbackCopyWorkspaceSummary(text, done); });
  } else {
    fallbackCopyWorkspaceSummary(text, done);
  }
}
