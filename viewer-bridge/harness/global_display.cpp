#include "../ksa64_viewer_bridge.h"
#include "../viewer_bridge_global_v1.h"
#include "platform.hpp"
#include "sha256.hpp"
#include <algorithm>
#include <array>
#include <chrono>
#include <cctype>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <sstream>
#include <stdexcept>
#include <string>
#include <thread>
#include <vector>

namespace {
constexpr uint32_t DIR=5,GUIDED=2,N=22015,LAST=22014,DIR_MASK=11,PUBLIC_MASK=3;
constexpr std::array<uint32_t,9> IMPORTANT{29,1920,3579,8124,12669,15255,15257,20929,22014};
[[noreturn]] void die(const std::string&s){throw std::runtime_error(s);}
void ok(bool v,const std::string&s){if(!v)die(s);}
template<class T>T sym(ksa64::native::LibraryHandle h,const char*n){return ksa64::native::required_symbol<T>(h,n);}
uint16_t r16(const uint8_t*p){return uint16_t(p[0])|uint16_t(p[1])<<8U;}
uint32_t r32(const uint8_t*p){return uint32_t(p[0])|uint32_t(p[1])<<8U|uint32_t(p[2])<<16U|uint32_t(p[3])<<24U;}
uint64_t r64(const uint8_t*p){return uint64_t(r32(p))|uint64_t(r32(p+4))<<32U;}
class R{
 const uint8_t*p;size_t n,a=12;
public:
 R(const std::vector<uint8_t>&b,const char*m):p(b.data()),n(b.size()){
  ok(n>=12&&std::memcmp(p,m,4)==0&&r16(p+4)==1&&r16(p+6)==12&&r32(p+8)==n,"bad global payload framing");
 }
 const uint8_t*t(size_t c){ok(c<=n-a,"truncated global payload");const uint8_t*q=p+a;a+=c;return q;}
 uint8_t b(){return t(1)[0];} uint16_t w(){return r16(t(2));} uint32_t d(){return r32(t(4));} uint64_t q(){return r64(t(8));}
 void z(size_t c){const uint8_t*q=t(c);ok(std::all_of(q,q+c,[](uint8_t x){return x==0;}),"nonzero reserved field");}
 void s(size_t c){(void)t(c);} void end()const{ok(a==n,"global payload has trailing data");}
};
Ksa64ViewerOwnedBuffer out(){Ksa64ViewerOwnedBuffer b{};b.abi_version=1;b.struct_size=sizeof(b);return b;}
using Free=int32_t(KSA64_VIEWER_CALL*)(Ksa64ViewerOwnedBuffer*);
std::vector<uint8_t> take(int32_t code,Ksa64ViewerOwnedBuffer&b,Free f){
 ok(code==KSA64_VIEWER_OK,"global API call failed");
 ok(b.abi_version==1&&b.struct_size==sizeof(b)&&b.data&&b.length>0&&b.length<=256U*1024U,"bad global owned buffer");
 std::vector<uint8_t>v(b.data,b.data+size_t(b.length));ok(f(&b)==0,"global buffer free failed");return v;
}
std::string hex(const std::array<uint8_t,32>&x){std::ostringstream o;o<<std::hex<<std::setfill('0');for(auto b:x)o<<std::setw(2)<<unsigned(b);return o.str();}
std::string hashfile(const std::filesystem::path&p){std::ifstream f(p,std::ios::binary);ok(f.good(),"cannot read "+p.string());std::vector<uint8_t>b((std::istreambuf_iterator<char>(f)),{});return hex(ksa64::crypto::sha256(b.data(),b.size()));}
std::string text(const std::filesystem::path&p){std::ifstream f(p);ok(f.good(),"cannot read manifest");return std::string((std::istreambuf_iterator<char>(f)),{});}
std::string field(const std::string&j,const std::string&k){
 auto x=j.find("\""+k+"\""),c=j.find(':',x);ok(x!=std::string::npos&&c!=std::string::npos,"missing manifest field "+k);
 auto b=j.find('"',c+1),e=j.find('"',b+1);ok(b!=std::string::npos&&e!=std::string::npos,"bad manifest field "+k);return j.substr(b+1,e-b-1);
}
struct Times{std::vector<uint64_t>v;void add(std::chrono::steady_clock::duration d){v.push_back(uint64_t(std::chrono::duration_cast<std::chrono::nanoseconds>(d).count()));}
 uint64_t p99()const{ok(!v.empty(),"empty timing");auto s=v;std::sort(s.begin(),s.end());return s.at((s.size()*99+98)/100-1);}
 uint64_t max()const{return *std::max_element(v.begin(),v.end());}};
struct S{
 uint32_t release,time,mask,event,discontinuity,continuity;
 uint8_t frame,segment;
};
std::vector<S> samples(const std::vector<uint8_t>&b,uint32_t allowed){
 R r(b,"PGS1");auto n=r.w();r.z(2);ok(n>0&&n<=256,"bad sample count");std::vector<S>o;uint64_t ps=0;
 for(uint16_t i=0;i<n;++i){uint64_t seq=r.q();uint32_t rel=r.d(),time=r.d();uint8_t frame=r.b(),seg=r.b(),mode=r.b(),tc=r.b();auto event=r.w();auto sc=r.w();auto discontinuity=r.d();auto ci=r.d();
  ok(seq>ps&&seq&&frame>=1&&frame<=3&&seg>=1&&seg<=5&&mode<=7&&tc<=4&&sc>0&&sc<=4&&ci&&time==rel*2048U,"bad global sample");ps=seq;r.s(12+24+4+12+4);uint32_t mask=0;
  for(uint16_t j=0;j<sc;++j){uint8_t source=r.b(),sf=r.b();r.z(2);uint32_t valid=r.d(),model=r.d();r.s(12);auto bit=1U<<(source-1U);
   ok(source>=1&&source<=4&&sf>=1&&sf<=3&&valid&&model&&(mask&bit)==0&&(allowed&bit),"bad/forbidden source");mask|=bit;r.s(212);}
  o.push_back({rel,time,mask,event,discontinuity,ci,frame,seg});}
 r.end();return o;
}
void definition(const std::vector<uint8_t>&b,uint32_t allowed){
 R r(b,"PGD1");uint32_t a=r.d(),e=r.d(),t=r.d(),m=r.d();r.s(6);r.z(2);r.s(12);auto la=r.d();r.s(24);auto ra=r.d();r.s(24);uint32_t src=r.d();uint8_t frames=r.b();r.z(1);auto cams=r.w();r.end();
 ok(a&&e&&t&&m&&la&&ra&&src==allowed&&frames==7&&cams,"bad role-filtered definition");
}
uint32_t transition(const std::vector<uint8_t>&b){
 R r(b,"PGT1");uint32_t rel=r.d(),time=r.d();uint8_t ff=r.b(),tf=r.b(),fs=r.b(),ts=r.b(),reason=r.b();r.z(3);auto id=r.d(),tr=r.d();r.s(20);auto crc=r.d();r.end();
 ok(rel&&time==rel*2048U&&ff>=1&&ff<=3&&tf>=1&&tf<=3&&ff!=tf&&fs>=1&&fs<=5&&ts>=1&&ts<=5&&fs!=ts&&reason&&id&&tr&&crc,"bad transition");return rel;
}
uint8_t index(const std::vector<uint8_t>&b,std::array<bool,IMPORTANT.size()>&hit){
 R r(b,"PGI1");uint32_t id=r.d(),def=r.d(),first=r.d(),last=r.d();uint8_t terminal=r.b();r.s(6);r.z(1);auto n=r.w();r.z(2);ok(id&&def&&first==0&&last==LAST&&terminal<=5&&n,"bad replay index");uint32_t pr=0,pt=0;
 for(uint16_t i=0;i<n;++i){uint32_t rel=r.d(),time=r.d();uint8_t kind=r.b();r.z(3);r.s(4);auto ev=r.d();r.s(4);ok(rel>=pr&&time>=pt&&rel<=LAST&&time==rel*2048U&&kind>=1&&kind<=5&&ev,"bad replay entry");pr=rel;pt=time;for(size_t j=0;j<hit.size();++j)hit[j]=hit[j]||rel==IMPORTANT[j];}r.end();return terminal;
}
struct PathStats{
 uint32_t identity,model,continuity,first_release,last_release,first_time,last_time;
 uint16_t chunk_index,chunk_count,points;
 uint8_t lod;
 size_t bytes;
};
PathStats path(const std::vector<uint8_t>&b,uint32_t es,uint32_t ef,uint32_t el){
 R r(b,"PGP1");uint32_t id=r.d();uint8_t s=r.b(),f=r.b(),l=r.b();r.z(1);uint16_t flags=r.w(),ci=r.w(),cc=r.w(),n=r.w();uint32_t model=r.d();r.s(8);auto cont=r.d();
 ok(id&&s==es&&f==ef&&l==el&&(flags&~15U)==0&&cc&&ci<cc&&n&&n<=4096&&model&&cont,"bad path header");uint32_t pr=0,pt=0,first_release=0,first_time=0;
 for(uint16_t i=0;i<n;++i){uint32_t rel=r.d(),time=r.d();uint8_t seg=r.b();r.z(1);r.s(18);ok((i==0||(rel>pr&&time>pt))&&rel<=LAST&&time==rel*2048U&&seg>=1&&seg<=5,"bad path point");if(i==0){first_release=rel;first_time=time;}pr=rel;pt=time;}r.end();return {id,model,cont,first_release,pr,first_time,pt,ci,cc,n,l,b.size()};
}
Ksa64GlobalDisplayAvailabilityV1 ready(const Ksa64GlobalDisplayApiV1&a,Ksa64ViewerHandle*h,Times&ts){
 auto limit=std::chrono::steady_clock::now()+std::chrono::minutes(5);for(;;){Ksa64GlobalDisplayAvailabilityV1 x{};x.api_version=1;x.struct_size=sizeof(x);auto b=std::chrono::steady_clock::now();auto code=a.availability(h,&x);ts.add(std::chrono::steady_clock::now()-b);
  if(code==0)return x;ok(code==KSA64_VIEWER_NO_DATA||code==KSA64_VIEWER_UNCHANGED,"global worker failed");ok(std::chrono::steady_clock::now()<limit,"global worker readiness window expired");std::this_thread::sleep_for(std::chrono::milliseconds(5));}
}
Ksa64ViewerHandle* start(const Ksa64GlobalDisplayApiV1&a,uint32_t role){Ksa64GlobalDisplayReplayStartRequestV1 r{};r.api_version=1;r.struct_size=sizeof(r);r.role=role;r.flags=KSA64_GLOBAL_DISPLAY_REPLAY_READ_ONLY;Ksa64ViewerHandle*h=nullptr;ok(a.start_nominal_replay(&r,&h)==0&&h,"nominal replay start failed");return h;}
struct StorageStats{
 size_t definition=0,samples=0,transitions=0,index=0,paths=0;
 size_t exact_bytes=0,exact_chunks=0,exact_points=0,exact_max_bytes=0,exact_max_points=0;
 size_t total()const{return definition+samples+transitions+index+paths;}
};
void report(const std::filesystem::path&o,const std::filesystem::path&dll,const std::filesystem::path&man,const std::string&commit,const std::string&catalog,const std::string&sem,const Times&av,const Times&ra,const Times&pa,size_t path_series,size_t path_chunks,uint8_t terminal,const std::array<S,IMPORTANT.size()>&milestones,const StorageStats&storage){
 std::ofstream f(o);ok(f.good(),"cannot write evidence");auto slash=[](std::string s){std::replace(s.begin(),s.end(),'\\','/');return s;};
 f<<"{\n  \"schema\":\"ksa64.phase12c.global-display-harness.v1\",\n  \"pass\":true,\n"
  <<"  \"bridge\":{\"path\":\""<<slash(dll.string())<<"\",\"sha256\":\""<<hashfile(dll)<<"\",\"manifest_path\":\""<<slash(man.string())<<"\",\"manifest_sha256\":\""<<hashfile(man)<<"\",\"source_commit\":\""<<commit<<"\",\"catalog_identity\":\""<<catalog<<"\"},\n"
  <<"  \"api\":{\"version\":1,\"table_size\":"<<sizeof(Ksa64GlobalDisplayApiV1)<<",\"availability_size\":"<<sizeof(Ksa64GlobalDisplayAvailabilityV1)<<",\"range_request_size\":"<<sizeof(Ksa64GlobalDisplaySampleRangeRequestV1)<<",\"path_request_size\":"<<sizeof(Ksa64GlobalDisplayPathRequestV1)<<"},\n"
  <<"  \"nominal_replay\":{\"samples\":22015,\"first_release\":0,\"last_release\":22014,\"transitions\":4,\"director_source_mask\":11,\"guided_source_mask\":3,\"successful_path_requests\":"<<path_series<<",\"successful_path_chunk_fetches\":"<<path_chunks<<",\"terminal_disposition\":"<<unsigned(terminal)<<",\"semantic_sha256\":\""<<sem<<"\"},\n"
  <<"  \"display_storage\":{\"measurement\":\"serialized GlobalDisplayV1 payload bytes\",\"definition_bytes\":"<<storage.definition<<",\"sample_bytes\":"<<storage.samples<<",\"transition_bytes\":"<<storage.transitions<<",\"replay_index_bytes\":"<<storage.index<<",\"path_bytes\":"<<storage.paths<<",\"nominal_replay_bytes\":"<<storage.total()<<",\"exact_path_storage\":{\"chunk_count\":"<<storage.exact_chunks<<",\"point_count\":"<<storage.exact_points<<",\"serialized_bytes\":"<<storage.exact_bytes<<"},\"exact_active_window_path\":{\"chunk_count\":1,\"point_count\":"<<storage.exact_max_points<<",\"serialized_bytes\":"<<storage.exact_max_bytes<<"}},\n"
  <<"  \"milestones\":[";
 for(size_t i=0;i<milestones.size();++i){const auto&s=milestones[i];if(i)f<<",";
  f<<"{\"release_epoch\":"<<s.release<<",\"mission_time_q16\":"<<s.time<<",\"frame_identity\":"<<unsigned(s.frame)<<",\"segment_identity\":"<<unsigned(s.segment)<<",\"source_mask\":"<<s.mask<<",\"event_mask\":"<<s.event<<",\"discontinuity_mask\":"<<s.discontinuity<<",\"continuity_identity\":"<<s.continuity<<"}";}
 f<<"],\n"
  <<"  \"timing\":{\"method\":\"steady_clock nearest-rank p99; direct C API calls only\",\"availability_samples\":"<<av.v.size()<<",\"availability_p99_ns\":"<<av.p99()<<",\"availability_max_ns\":"<<av.max()<<",\"range_samples\":"<<ra.v.size()<<",\"range_p99_ns\":"<<ra.p99()<<",\"range_max_ns\":"<<ra.max()<<",\"path_samples\":"<<pa.v.size()<<",\"path_p99_ns\":"<<pa.p99()<<",\"path_max_ns\":"<<pa.max()<<"}\n}\n";ok(f.good(),"evidence write failed");
}
} // namespace
int main(int argc,char**argv){try{
 std::filesystem::path dll=argc>1?argv[1]:ksa64::native::kDefaultBridgePath,man=argc>2?argv[2]:dll.string()+".json",output=argc>3?argv[3]:"phase12c-global-display-evidence.json";
 ok(std::filesystem::exists(dll)&&std::filesystem::exists(man),"bridge DLL or manifest missing");auto mt=text(man);ok(mt.find("ksa64.viewer-bridge-artifact.v2")!=std::string::npos,"v2 manifest required");auto commit=field(mt,"source_commit"),dh=field(mt,"sha256"),catalog=field(mt,"catalog_identity");ok(dh==hashfile(dll),"bridge hash mismatches manifest");
 auto lib=ksa64::native::open_library(dll.string().c_str());ok(lib,"dynamic bridge load failed: "+ksa64::native::loader_error());auto close=[&]{if(lib){ksa64::native::close_library(lib);lib=nullptr;}};
 auto api_entry=sym<int32_t(KSA64_VIEWER_CALL*)(Ksa64GlobalDisplayApiV1*)>(lib,"ksa64_viewer_global_display_api_v1");auto free=sym<Free>(lib,"ksa64_viewer_free_buffer");auto destroy=sym<int32_t(KSA64_VIEWER_CALL*)(Ksa64ViewerHandle*)>(lib,"ksa64_viewer_destroy");
 Ksa64GlobalDisplayApiV1 api{};api.api_version=1;api.struct_size=sizeof(api);ok(api_entry(&api)==0&&api.api_version==1&&api.struct_size==sizeof(api)&&(api.feature_flags&(KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED|KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED))==(KSA64_GLOBAL_DISPLAY_API_IMPLEMENTED|KSA64_GLOBAL_DISPLAY_API_ROLE_FILTERED)&&api.replay_start_request_size==sizeof(Ksa64GlobalDisplayReplayStartRequestV1)&&api.availability_size==sizeof(Ksa64GlobalDisplayAvailabilityV1)&&api.path_request_size==sizeof(Ksa64GlobalDisplayPathRequestV1)&&api.sample_range_request_size==sizeof(Ksa64GlobalDisplaySampleRangeRequestV1)&&api.owned_buffer_size==sizeof(Ksa64ViewerOwnedBuffer)&&api.start_nominal_replay&&api.availability&&api.definition_payload&&api.sample_range_payload&&api.poll_transition_payload&&api.replay_index_payload&&api.path_chunk_payload,"incomplete GlobalDisplay API");
 Times av,ra,pa;StorageStats storage;auto*dir=start(api,DIR);try{
  auto a=ready(api,dir,av);ok(a.role==DIR&&a.sample_count==N&&a.transition_count==4&&a.oldest_sample_release==0&&a.newest_sample_release==LAST&&a.available_source_mask==DIR_MASK&&a.available_frame_mask==7&&(a.flags&KSA64_GLOBAL_DISPLAY_AVAILABILITY_ACCEPTED_EXACT),"bad director availability");
  auto b=out();auto def=take(api.definition_payload(dir,&b),b,free);storage.definition=def.size();definition(def,DIR_MASK);b=out();auto idx=take(api.replay_index_payload(dir,&b),b,free);storage.index=idx.size();std::array<bool,IMPORTANT.size()>ih{};auto terminal=index(idx,ih);std::array<bool,IMPORTANT.size()>sh{};std::array<S,IMPORTANT.size()>milestones{};std::vector<uint8_t>sem;sem.insert(sem.end(),def.begin(),def.end());sem.insert(sem.end(),idx.begin(),idx.end());uint32_t expected=0;
  for(uint32_t startrel=0;startrel<N;startrel+=256){Ksa64GlobalDisplaySampleRangeRequestV1 q{};q.api_version=1;q.struct_size=sizeof(q);q.start_release=startrel;q.max_count=std::min<uint32_t>(256,N-startrel);b=out();auto begun=std::chrono::steady_clock::now();auto code=api.sample_range_payload(dir,&q,&b);ra.add(std::chrono::steady_clock::now()-begun);auto payload=take(code,b,free);storage.samples+=payload.size();auto ss=samples(payload,DIR_MASK);ok(ss.size()==q.max_count,"range sample count mismatch");for(auto s:ss){ok(s.release==expected++,"exact release ordering mismatch");for(size_t j=0;j<IMPORTANT.size();++j){if(s.release==IMPORTANT[j]){sh[j]=true;milestones[j]=s;}}}sem.insert(sem.end(),payload.begin(),payload.end());}
  ok(expected==N&&std::all_of(ih.begin(),ih.end(),[](bool x){return x;})&&std::all_of(sh.begin(),sh.end(),[](bool x){return x;}),"important release omitted");for(auto er:std::array<uint32_t,4>{29,3579,12669,15255}){b=out();auto p=take(api.poll_transition_payload(dir,&b),b,free);storage.transitions+=p.size();ok(transition(p)==er,"transition mismatch");sem.insert(sem.end(),p.begin(),p.end());}b=out();ok(api.poll_transition_payload(dir,&b)==KSA64_VIEWER_NO_DATA&&!b.data&&!b.length,"unexpected transition");
  size_t path_series=0,path_chunks=0;for(uint32_t source:std::array<uint32_t,3>{1,2,4}){bool found=false;for(uint32_t frame=1;frame<=3;++frame){for(uint32_t lod=1;lod<=3;++lod){bool series_found=false;uint32_t chunk=0,expected_chunks=0,path_identity=0,path_model=0,path_continuity=0,previous_release=0,previous_time=0;while(chunk<(expected_chunks==0?1:expected_chunks)){Ksa64GlobalDisplayPathRequestV1 q{};q.api_version=1;q.struct_size=sizeof(q);q.source=source;q.display_frame=frame;q.lod=lod;q.chunk_index=chunk;b=out();auto begun=std::chrono::steady_clock::now();auto code=api.path_chunk_payload(dir,&q,&b);pa.add(std::chrono::steady_clock::now()-begun);if(code==KSA64_VIEWER_NO_DATA){ok(chunk==0&&!b.data&&!b.length,"path no-data after initial chunk");break;}auto p=take(code,b,free);auto ps=path(p,source,frame,lod);ok(ps.chunk_index==chunk,"path chunk ordering mismatch");if(chunk==0){expected_chunks=ps.chunk_count;path_identity=ps.identity;path_model=ps.model;path_continuity=ps.continuity;}else{ok(ps.chunk_count==expected_chunks&&ps.identity==path_identity&&ps.model==path_model&&ps.continuity==path_continuity,"path chunk metadata changed within series");ok(ps.first_release>previous_release&&ps.first_time>previous_time,"path chunk boundary is duplicate or nonmonotonic");}previous_release=ps.last_release;previous_time=ps.last_time;storage.paths+=ps.bytes;if(lod==1){storage.exact_bytes+=ps.bytes;++storage.exact_chunks;storage.exact_points+=ps.points;storage.exact_max_bytes=std::max(storage.exact_max_bytes,ps.bytes);storage.exact_max_points=std::max(storage.exact_max_points,size_t(ps.points));}sem.insert(sem.end(),p.begin(),p.end());found=true;series_found=true;++path_chunks;++chunk;}if(series_found){ok(chunk==expected_chunks,"path series ended before every declared chunk");++path_series;}}}ok(found,"permitted source has no path");}ok(storage.total()>storage.samples&&storage.exact_bytes&&storage.exact_chunks&&storage.exact_points,"display storage accounting is incomplete");
  auto*guided=start(api,GUIDED);try{Times gt;auto ga=ready(api,guided,gt);ok(ga.role==GUIDED&&ga.available_source_mask==PUBLIC_MASK&&ga.sample_count==N,"guided role filter invalid");b=out();auto gd=take(api.definition_payload(guided,&b),b,free);definition(gd,PUBLIC_MASK);Ksa64GlobalDisplaySampleRangeRequestV1 q{};q.api_version=1;q.struct_size=sizeof(q);q.start_release=3579;q.max_count=1;b=out();auto gs=take(api.sample_range_payload(guided,&q,&b),b,free);auto one=samples(gs,PUBLIC_MASK);ok(one.size()==1&&(one[0].mask&~PUBLIC_MASK)==0&&(one[0].mask&2U)!=0,"guided sample contains truth or omits onboard pose");Ksa64GlobalDisplayPathRequestV1 tq{};tq.api_version=1;tq.struct_size=sizeof(tq);tq.source=4;tq.display_frame=2;tq.lod=3;b=out();ok(api.path_chunk_payload(guided,&tq,&b)==KSA64_VIEWER_UNSUPPORTED&&!b.data&&!b.length,"guided truth path accepted");}catch(...){(void)destroy(guided);throw;}ok(destroy(guided)==0,"guided destroy failed");
  report(output,dll,man,commit,catalog,hex(ksa64::crypto::sha256(sem.data(),sem.size())),av,ra,pa,path_series,path_chunks,terminal,milestones,storage);ok(destroy(dir)==0,"director destroy failed");dir=nullptr;
 }catch(...){if(dir)(void)destroy(dir);throw;}close();std::cout<<"KSA64 GlobalDisplayApiV1 harness passed: 22015 exact samples, transitions, role filtering, paths, index, and hash-bound JSON\n";return 0;
}catch(const std::exception&e){std::cerr<<"KSA64 GlobalDisplayApiV1 harness failed: "<<e.what()<<"\n";return 1;}}