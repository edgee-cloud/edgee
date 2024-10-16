(()=>{var j=Object.defineProperty;var v=Object.getOwnPropertySymbols;var D=Object.prototype.hasOwnProperty,T=Object.prototype.propertyIsEnumerable;var y=(e,t,o)=>t in e?j(e,t,{enumerable:!0,configurable:!0,writable:!0,value:o}):e[t]=o,l=(e,t)=>{for(var o in t||(t={}))D.call(t,o)&&y(e,o,t[o]);if(v)for(var o of v(t))T.call(t,o)&&y(e,o,t[o]);return e};var I={eventPath:"/_edgee/event",asThirdPartySdk:!1,methods:["user","track","page"]},r=I;var W=e=>{let t=window.location.search;return new URLSearchParams(t).get(e)},h=()=>{let e=W("_edgeedebug");if(e!==null){if(e==="true")return document.cookie="_edgeedebug=true; path=/",!0;if(e==="false")return document.cookie="_edgeedebug=false; path=/",!1}return!!document.cookie.includes("_edgeedebug=true")},u=(...e)=>{typeof e!="object"&&(e=[e]),console&&typeof console.log=="function"&&h()&&console.log("%cEDGEE","display: inline-block; color: #61d2a3; background: #231A26; padding: 1px 4px; border-radius: 3px;",...e)},p=(...e)=>{typeof e!="object"&&(e=[e]),console&&typeof console.error=="function"&&h()&&console.error("%cEDGEE","display: inline-block; color: #CB134A; background: #231A26; padding: 1px 4px; border-radius: 3px;",...e)};function q(e){let t=r.eventPath;r.asThirdPartySdk&&typeof localStorage!="undefined"&&localStorage.getItem("_edgee")&&(t=t+"?e="+localStorage.getItem("_edgee"));let o=JSON.stringify(e),n={"Content-Type":"application/json"};h()&&(n["Edgee-Debug"]="1"),fetch(t,{method:"POST",headers:n,body:o}).then(c=>{c.status!==200&&c.status!==204?p("Failed to send event to "+r.eventPath+": "+c.status):c.status===200&&c.json().then(a=>{r.asThirdPartySdk&&typeof localStorage!="undefined"?(a.e&&localStorage.setItem("_edgee",a.e),a.events&&a.events.length>0&&u("* client-side events:",a.events)):a.length>0&&u("* client-side events:",a)}).catch(a=>{p("Failed to parse response: "+a)})}).catch(c=>{p("Failed to send event to "+r.eventPath+": "+c)})}var m=q;function f(){return L().then(e=>{let t={};e!==null&&(t=JSON.parse(e.textContent)),t.data_collection=t.data_collection||{},t.data_collection.context=t.data_collection.context||{},t.data_collection.events=t.data_collection.events||[];let o=Intl.DateTimeFormat().resolvedOptions().timeZone;o&&(t.data_collection.context.client={},t.data_collection.context.client.timezone=o);let n=window.screen?window.screen.width:0,c=window.screen?window.screen.height:0;n&&c&&(t.data_collection.context.client=t.data_collection.context.client||{},t.data_collection.context.client.screen_width=n,t.data_collection.context.client.screen_height=c);let a=window.devicePixelRatio;a&&(t.data_collection.context.client=t.data_collection.context.client||{},t.data_collection.context.client.screen_density=a);let i=new URLSearchParams(window.location.search);i.has("utm_campaign")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.name=i.get("utm_campaign")),i.has("utm_source")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.source=i.get("utm_source")),i.has("utm_medium")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.medium=i.get("utm_medium")),i.has("utm_term")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.term=i.get("utm_term")),i.has("utm_content")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.content=i.get("utm_content")),i.has("utm_creative_format")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.creative_format=i.get("utm_creative_format")),i.has("utm_marketing_tactic")&&(t.data_collection.context.campaign=t.data_collection.context.campaign||{},t.data_collection.context.campaign.marketing_tactic=i.get("utm_marketing_tactic"));let d;document.querySelector('link[rel="canonical"]')&&document.querySelector('link[rel="canonical"]').getAttribute("href")&&(d=document.querySelector('link[rel="canonical"]').getAttribute("href"),!d.startsWith("https://")&&!d.startsWith("http://")&&!d.startsWith("//")&&(d=window.location.protocol+"//"+window.location.host+d));let s;if(d){s=d.replace(/^https?:\/\//,"");let _=s.split("/")[0];s=s.replace(_,""),s=s.split("?")[0]}if(t.data_collection.context.page=t.data_collection.context.page||{},t.data_collection.context.page.url||(d?t.data_collection.context.page.url=d:t.data_collection.context.page.url=window.location.protocol+"//"+window.location.host+window.location.pathname+window.location.search),t.data_collection.context.page.path||(s?t.data_collection.context.page.path=s:t.data_collection.context.page.path=window.location.pathname),!t.data_collection.context.page.search&&window.location.search!==""&&(t.data_collection.context.page.search=window.location.search),t.data_collection.context.page.title||(t.data_collection.context.page.title=document.title),!t.data_collection.context.page.keywords){let _=document.querySelector('meta[name="keywords"]');if(_){let E=_.getAttribute("content");t.data_collection.context.page.keywords=E.split(",").map(A=>A.trim())}}return document.referrer&&(t.data_collection.context.page.referrer=document.referrer),t})}function L(){return new Promise(e=>{function t(){let o=document.getElementById("__EDGEE_DATA_LAYER__");o!==null?e(o):document.readyState==="complete"?e(null):document.onreadystatechange=()=>{document.readyState==="complete"&&t()}}t()})}function F(e){f().then(t=>{t.data_collection.events=[];let o={};if(o.type="page",e.length!==0){let[n,c]=e;typeof n=="string"?o.data=n:typeof n=="object"&&(o.data=l(l({},t.data_collection.context.page),n)),typeof c=="object"&&(o.components=l(l({},t.data_collection.components),c))}t.data_collection.events.push(o),m(t)})}var b=F;function O(e){let t="Event name is required to track an event";if(e.length===0){p(t);return}f().then(o=>{o.data_collection.events=[];let n={};n.type="track",n.data={};let[c,a]=e;if(typeof c=="string")n.data.name=c;else if(typeof c=="object"){if(!c.name){p(t);return}n.data=c}if(c.name===""){p(t);return}typeof a=="object"&&(n.components=l(l({},o.data_collection.components),a)),o.data_collection.events.push(n),m(o)})}var k=O;function R(e){f().then(t=>{t.data_collection.events=[];let o={};if(o.type="user",e.length!==0){let[n,c]=e;typeof n=="string"?o.data.userId=n:typeof n=="object"&&(o.data=l(l({},t.data_collection.context.user),n)),typeof c=="object"&&(o.components=l(l({},t.data_collection.components),c))}t.data_collection.events.push(o),m(t)})}var P=R;function C(){f().then(e=>{if(e.data_collection.events.length===0){let t={};t.type="page",e.data_collection.events.push(t)}m(e)})}var S=C;var g=window.edgee=window.edgee||[];if(!g.load){for(g.load=!0,g.factory=function(n){return function(){let c=Array.prototype.slice.call(arguments);return G(n,c),g}},x=0;x<r.methods.length;x++)w=r.methods[x],g[w]=g.factory(w);let e=document.currentScript,t=e.getAttribute("data-event-path");if(t)u("- Event path set to "+t),r.eventPath=t;else{r.asThirdPartySdk=!0;let n=e.src;if(n&&(n.startsWith("http://")||n.startsWith("https://")||n.startsWith("//"))){n=n.replace("https://",""),n=n.replace("http://",""),n=n.replace("^//","");let c=n.split("/")[0];r.eventPath=`https://${c}/_edgee/csevent`,u("- Edgee SDK used as third party. Event path set to "+r.eventPath)}}e.getAttribute("data-client-side")==="true"?S():typeof window._edgee_events=="object"&&window._edgee_events.length>0&&u("* edge-side events:",window._edgee_events),window.dispatchEvent(new Event("edgee:loaded"))}var w,x;function G(e,t){e==="page"?b(t):e==="track"?k(t):e==="user"&&P(t)}})();