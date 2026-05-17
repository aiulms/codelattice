// live.js — CodeLattice Live MCP Mode (Phase G)
var LIVE=window.LIVE||{}; window.LIVE=LIVE;
LIVE.mcpAvailable=false; LIVE.tools=[]; LIVE.jobs=[]; LIVE.pollInterval=null;

function livePollJobs(){
  if(!RUNNER.connected){if(LIVE.pollInterval)clearInterval(LIVE.pollInterval); return;}
  rapi("/api/mcp/jobs").then(function(d){
    var jobs=Array.isArray(d.data)?d.data:((d.data&&Array.isArray(d.data.jobs))?d.data.jobs:[]);
    LIVE.jobs=jobs.slice(-20);
    renderLiveJobs();
  });
}
function liveCheckMcp(){
  if(!RUNNER.connected)return;
  rapi("/api/mcp/status").then(function(d){
    LIVE.mcpAvailable=d.data.available; LIVE.toolCount=d.data.toolCount||0;
    renderLiveStatus();
  }).catch(function(){LIVE.mcpAvailable=false; renderLiveStatus();});
}
function liveLoadTools(){if(!RUNNER.connected)return;
  rapi("/api/mcp/tools").then(function(d){
    LIVE.tools=Array.isArray(d.data)?d.data:((d.data&&Array.isArray(d.data.tools))?d.data.tools:[]);
    renderLiveTools();
  });}

function renderLiveStatus(){
  var el=document.getElementById("live-mcp-status"); if(!el)return;
  if(!RUNNER.connected){el.innerHTML='<span class="text-muted">Start runner for Live MCP.</span>'; return;}
  var cls=LIVE.mcpAvailable?"badge-success":"badge-danger";
  el.innerHTML='<span class="badge '+cls+'">'+(LIVE.mcpAvailable?"MCP connected ("+LIVE.toolCount+" tools)":"MCP unavailable")+'</span>';
}

function liveCreateJob(){
  if(!RUNNER.connected||!LIVE.mcpAvailable){alert("MCP not available.");return;}
  var wf=document.getElementById("live-workflow-select").value;
  var root=document.getElementById("runner-root-input").value.trim();
  var lang=document.getElementById("runner-lang-select").value;
  if(!root){alert("Enter project root or select a profile.");return;}
  var params={};
  if(wf==="symbol_search") params.query=document.getElementById("live-symbol-query").value||"main";
  if(wf==="impact_preview") params.symbol=document.getElementById("live-symbol-query").value||"";
  var pid=RUNNER.selectedProfile||"";
  var st=document.getElementById("live-job-status");
  if(st)st.textContent="Creating job...";
  rapi("/api/mcp/jobs",{method:"POST",body:{root:root,language:lang,workflow:wf,profileId:pid,params:params,redactRoot:true}}).then(function(d){
    if(st)st.textContent="Job created: "+d.data.id; livePollJobs();
  }).catch(function(e){if(st)st.textContent="Error: "+e.message;});
}

function liveCancelJob(jid){if(!RUNNER.connected)return;
  rapi("/api/mcp/job/"+jid+"/cancel",{method:"POST"}).then(function(){livePollJobs();});}
function liveDeleteJob(jid){if(!RUNNER.connected||!confirm("Delete job?"))return;
  rapi("/api/mcp/job/"+jid,{method:"DELETE"}).then(function(){livePollJobs();});}

function renderLiveJobs(){
  var el=document.getElementById("live-jobs-list"); if(!el)return;
  var jobs=LIVE.jobs;
  if(jobs.length===0){el.innerHTML='<span class="text-muted text-sm">No live jobs yet.</span>'; return;}
  el.innerHTML=jobs.map(function(j){
    var statusBadge={"queued":"badge-info","running":"badge-warning","succeeded":"badge-success","failed":"badge-danger","cancelled":"badge-info"};
    return '<div class="gate-item" style="font-size:0.82em;flex-wrap:wrap;">'+
      '<span><span class="badge '+(statusBadge[j.status]||"badge-info")+'">'+j.status+'</span> '+
      '<strong>'+esc(j.workflow||j.tool)+'</strong> '+
      '<span class="text-muted">'+esc((j.createdAt||"").slice(11,19))+'</span></span>'+
      '<span>'+ (j.status==="succeeded"?'<button class="btn btn-sm btn-primary" onclick="liveViewResult(&quot;'+escAttr(j.id)+'&quot;)">Result</button> ':'')+
      (j.status==="queued"||j.status==="running"?'<button class="btn btn-sm btn-secondary" onclick="liveCancelJob(&quot;'+escAttr(j.id)+'&quot;)">Cancel</button> ':'')+
      '<button class="btn btn-sm btn-secondary" onclick="liveDeleteJob(&quot;'+escAttr(j.id)+'&quot;)" style="color:#dc2626;">x</button></span></div>';
  }).join("");
}

function liveViewResult(jid){
  if(!RUNNER.connected)return;
  rapi("/api/mcp/job/"+jid).then(function(d){
    var job=d.data; if(!job||!job.result){alert("No result yet.");return;}
    LIVE.lastJob=job; var r=job.result; var html='<h4>'+esc(job.workflow||job.tool)+' '+
      (job.status==="succeeded"?'<span class="badge badge-success">done</span>':'<span class="badge badge-danger">'+esc(job.status)+'</span>')+'</h4>';
    if(job.error){html+='<div class="caution-box">'+esc(job.error)+'</div>';}
    // Structured rendering by workflow
    try{
      var d=typeof r==="string"?JSON.parse(r):r;
      if(typeof d==="object"&&d!==null){
        if(job.workflow==="project_overview") html+=renderProjectOverview(d);
        else if(job.workflow==="symbol_search") html+=renderSymbolSearch(d);
        else if(job.workflow==="impact_preview") html+=renderImpactResult(d);
        else if(job.workflow==="dead_code_candidates") html+=renderDeadCodeCandidates(d);
        else html+='<pre class="code-block" style="max-height:400px;overflow:auto;font-size:.78em;">'+esc(JSON.stringify(d,null,2).slice(0,6000))+'</pre>';
      }else{html+='<pre class="code-block" style="max-height:300px;overflow:auto;">'+esc(String(r).slice(0,3000))+'</pre>';}
    }catch(e){html+='<pre class="code-block" style="max-height:300px;overflow:auto;">'+esc(String(r).slice(0,3000))+'</pre>';}
    html+='<div class="caution-box" style="margin-top:8px;"><strong>Static analysis only</strong> — not runtime proof.</div>';
    html+='<button class="btn btn-sm btn-secondary" onclick="liveIncludeInReport(&quot;'+jid+'&quot;)">Include in Report</button>';
    var el=document.getElementById("live-job-result"); if(el)el.innerHTML=html;
  });
}
function renderProjectOverview(d){
  var s=d.summary||d; return '<div class="card-grid card-grid-4" style="margin-top:8px;">'+
    ['sourceFileCount','symbolCount','edgeCount','nodeCount'].map(function(k){
      return '<div class="stat-card"><div class="stat-label">'+k+'</div><div class="stat-value">'+(s[k]||0)+'</div></div>';}).join("")+'</div>';}
function renderSymbolSearch(d){
  var c=d.candidates||d.data||[]; if(!c.length)return '<span class="text-muted">No matches.</span>';
  return '<div style="max-height:300px;overflow:auto;font-size:.85em;">'+c.slice(0,20).map(function(s){
    return '<div class="gate-item"><span><strong>'+esc(s.name||s.id)+'</strong> '+esc(s.kind||'')+'</span><span class="text-muted">'+esc(s.file||'')+'</span></div>';}).join("")+'</div>';}
function renderImpactResult(d){
  return '<div class="card-grid card-grid-3">'+
    '<div class="stat-card"><div class="stat-label">Risk</div><div class="stat-value">'+esc(d.risk||d.riskLevel||'?')+'</div></div>'+
    '<div class="stat-card"><div class="stat-label">Reasons</div><div class="stat-value" style="font-size:.85em;">'+(d.riskReasons||[]).map(esc).join('<br>')+'</div></div>'+
    '<div class="stat-card"><div class="stat-label">Files</div><div class="stat-value">'+(d.impactedFileCount||(d.impactMetrics||{}).impactedFileCount||'?')+'</div></div></div>';}
function renderDeadCodeCandidates(d){
  var c=d.candidateSymbols||d.candidates||[]; return '<div class="caution-box">NOT deletion-proof — verify manually.</div>'+
    '<div style="max-height:300px;overflow:auto;font-size:.85em;">'+c.slice(0,20).map(function(s){
      return '<div class="gate-item"><span>'+esc(s.name||s.id)+'</span><span class="badge '+(s.confidence==='high'?'badge-danger':'badge-warning')+'">'+esc(s.confidence||s.score)+'</span></div>';}).join("")+'</div>';}
function liveIncludeInReport(jid){
  LIVE.lastJobResultId=jid; if(typeof CTL!=="undefined"){CTL.selectedTemplate="general_snapshot_review"; show("report"); CTL.renderReport();}
}

function renderLiveTools(){var el=document.getElementById("live-tools-list"); if(!el)return;
  var tools=LIVE.tools; if(tools.length===0){el.innerHTML='';return;}
  el.innerHTML='<span class="text-muted text-sm">Available: '+tools.length+' tools</span>';}

// Init
document.addEventListener("DOMContentLoaded",function(){
  setTimeout(function(){liveCheckMcp();liveLoadTools(); livePollJobs(); LIVE.pollInterval=setInterval(livePollJobs,5000);},1000);
});
