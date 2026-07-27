#include "../ksa64_viewer_bridge.h"
#include <windows.h>
#include <bcrypt.h>
#include <array>
#include <cassert>
#include <chrono>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>
#include <vector>
#pragma comment(lib, "bcrypt.lib")

namespace {
constexpr uint32_t kCompleted=5,kAborted=6,kSingleStep=4;
std::array<uint8_t,32> sha256(const uint8_t*d,size_t n){BCRYPT_ALG_HANDLE a=nullptr;BCRYPT_HASH_HANDLE h=nullptr;DWORD z=0,r=0;std::array<uint8_t,32> out{};assert(BCryptOpenAlgorithmProvider(&a,BCRYPT_SHA256_ALGORITHM,nullptr,0)>=0);assert(BCryptGetProperty(a,BCRYPT_OBJECT_LENGTH,reinterpret_cast<PUCHAR>(&z),sizeof(z),&r,0)>=0);std::vector<uint8_t> obj(z);assert(BCryptCreateHash(a,&h,obj.data(),z,nullptr,0,0)>=0);assert(n<=ULONG_MAX);assert(BCryptHashData(h,const_cast<PUCHAR>(d),static_cast<ULONG>(n),0)>=0);assert(BCryptFinishHash(h,out.data(),static_cast<ULONG>(out.size()),0)>=0);BCryptDestroyHash(h);BCryptCloseAlgorithmProvider(a,0);return out;}
uint32_t crc32(const uint8_t*d,size_t n){uint32_t c=UINT32_MAX;for(size_t i=0;i<n;++i){c^=d[i];for(unsigned b=0;b<8;++b)c=(c>>1u)^(0xedb88320u&(0u-(c&1u)));}return ~c;}
void put32(uint8_t*p,uint32_t v){p[0]=static_cast<uint8_t>(v);p[1]=static_cast<uint8_t>(v>>8u);p[2]=static_cast<uint8_t>(v>>16u);p[3]=static_cast<uint8_t>(v>>24u);}
template<class T>T required(HMODULE d,const char*n){auto p=reinterpret_cast<T>(GetProcAddress(d,n));assert(p);return p;}
template<class T>T optional(HMODULE d,const char*n){return reinterpret_cast<T>(GetProcAddress(d,n));}
struct Api{
 int32_t(*abi)(Ksa64ViewerAbiInfo*);int32_t(*catalog)(Ksa64ViewerOwnedBuffer*);int32_t(*free_buffer)(Ksa64ViewerOwnedBuffer*);int32_t(*library_diagnostic)(Ksa64ViewerOwnedBuffer*);
 int32_t(*start)(const Ksa64ViewerSpan*,Ksa64ViewerHandle**);int32_t(*destroy)(Ksa64ViewerHandle*);
 int32_t(*pause)(const Ksa64ViewerHandle*);int32_t(*resume)(const Ksa64ViewerHandle*);int32_t(*set_pace)(const Ksa64ViewerHandle*,uint32_t);int32_t(*step)(const Ksa64ViewerHandle*);int32_t(*advance)(const Ksa64ViewerHandle*,uint32_t);int32_t(*abort)(const Ksa64ViewerHandle*,uint32_t);
 int32_t(*poll)(const Ksa64ViewerHandle*,Ksa64ViewerSnapshot*);int32_t(*event)(const Ksa64ViewerHandle*,Ksa64ViewerEvent*);
 int32_t(*recommended)(const Ksa64ViewerHandle*,Ksa64ViewerOwnedBuffer*);int32_t(*commit_request)(const Ksa64ViewerHandle*,Ksa64ViewerOwnedBuffer*);
 int32_t(*submit_stage)(const Ksa64ViewerHandle*,const Ksa64ViewerSpan*,uint32_t);int32_t(*submit_commit)(const Ksa64ViewerHandle*,const Ksa64ViewerSpan*);int32_t(*submit_cancel)(const Ksa64ViewerHandle*,const Ksa64ViewerSpan*);
 int32_t(*completed)(const Ksa64ViewerHandle*,Ksa64ViewerOwnedBuffer*);int32_t(*diagnostic)(const Ksa64ViewerHandle*,Ksa64ViewerOwnedBuffer*);int32_t(*panic_probe)(const Ksa64ViewerHandle*);
};
Ksa64ViewerOwnedBuffer empty_buffer(){Ksa64ViewerOwnedBuffer x{};x.abi_version=1;x.struct_size=sizeof(x);return x;}
Ksa64ViewerSpan make_span(const uint8_t*p,uint64_t n){Ksa64ViewerSpan x{};x.abi_version=1;x.struct_size=sizeof(x);x.data=p;x.length=n;return x;}
Ksa64ViewerSnapshot snap_header(){Ksa64ViewerSnapshot x{};x.abi_version=1;x.struct_size=sizeof(x);return x;}
Ksa64ViewerSnapshot wait_command(const Api&a,Ksa64ViewerHandle*h,uint64_t after){auto end=std::chrono::steady_clock::now()+std::chrono::seconds(5);for(;;){auto s=snap_header();int32_t r=a.poll(h,&s);if(r==0&&s.command_sequence>after)return s;assert(r==KSA64_VIEWER_OK||r==KSA64_VIEWER_NO_DATA||r==KSA64_VIEWER_UNCHANGED);assert(std::chrono::steady_clock::now()<end);std::this_thread::yield();}}
Ksa64ViewerSnapshot initial_snapshot(const Api&a,Ksa64ViewerHandle*h){auto end=std::chrono::steady_clock::now()+std::chrono::seconds(5);for(;;){auto s=snap_header();int32_t r=a.poll(h,&s);if(r==0)return s;assert(r==KSA64_VIEWER_NO_DATA||r==KSA64_VIEWER_UNCHANGED);assert(std::chrono::steady_clock::now()<end);std::this_thread::yield();}}
void drain_events(const Api&a,Ksa64ViewerHandle*h){Ksa64ViewerEvent e{};e.abi_version=1;e.struct_size=sizeof(e);uint32_t count=0,last=0;int32_t r;while((r=a.event(h,&e))==0){assert(count==0||e.sequence>last);last=e.sequence;++count;}assert(r==KSA64_VIEWER_EVENT_OVERFLOW||(r==KSA64_VIEWER_NO_DATA&&count>0));}
void cancel_abort(const Api&a,const Ksa64ViewerSpan&role){Ksa64ViewerHandle*h=nullptr;assert(a.start(&role,&h)==0);auto s=initial_snapshot(a,h);assert(a.set_pace(h,kSingleStep)==1);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0&&s.pace==kSingleStep);assert(a.step(h)==1);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0&&s.release_epoch==1);auto load=empty_buffer();assert(a.recommended(h,&load)==0&&load.length==512);auto p=make_span(load.data,load.length);assert(a.submit_stage(h,&p,0)==1);assert(a.free_buffer(&load)==0);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0);auto cancel=empty_buffer();assert(a.commit_request(h,&cancel)==0&&cancel.length==128);cancel.data[5]=4;put32(cancel.data+124,crc32(cancel.data,124));p=make_span(cancel.data,cancel.length);assert(a.submit_cancel(h,&p)==1);assert(a.free_buffer(&cancel)==0);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0);assert(a.abort(h,0x120aU)==1);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0&&s.lifecycle==kAborted);drain_events(a,h);assert(a.destroy(h)==0);auto stale=snap_header();assert(a.poll(h,&stale)==KSA64_VIEWER_INVALID_ARGUMENT);}
}

int main(int argc,char**argv){
 const char*path=argc>1?argv[1]:"..\\..\\target\\viewer\\ksa64_viewer_bridge.dll";HMODULE dll=LoadLibraryA(path);if(!dll){std::cerr<<"LoadLibrary failed: "<<GetLastError()<<"\n";return 2;}
 Api a{
  required<decltype(Api::abi)>(dll,"ksa64_viewer_get_abi_info"),required<decltype(Api::catalog)>(dll,"ksa64_viewer_catalog"),required<decltype(Api::free_buffer)>(dll,"ksa64_viewer_free_buffer"),required<decltype(Api::library_diagnostic)>(dll,"ksa64_viewer_library_diagnostic"),
  required<decltype(Api::start)>(dll,"ksa64_viewer_start"),required<decltype(Api::destroy)>(dll,"ksa64_viewer_destroy"),
  required<decltype(Api::pause)>(dll,"ksa64_viewer_pause"),required<decltype(Api::resume)>(dll,"ksa64_viewer_resume"),required<decltype(Api::set_pace)>(dll,"ksa64_viewer_set_pace"),required<decltype(Api::step)>(dll,"ksa64_viewer_step"),required<decltype(Api::advance)>(dll,"ksa64_viewer_advance"),required<decltype(Api::abort)>(dll,"ksa64_viewer_abort"),
  required<decltype(Api::poll)>(dll,"ksa64_viewer_poll_snapshot"),required<decltype(Api::event)>(dll,"ksa64_viewer_poll_event"),
  required<decltype(Api::recommended)>(dll,"ksa64_viewer_recommended_load"),required<decltype(Api::commit_request)>(dll,"ksa64_viewer_commit_request"),
  required<decltype(Api::submit_stage)>(dll,"ksa64_viewer_submit_stage"),required<decltype(Api::submit_commit)>(dll,"ksa64_viewer_submit_commit"),required<decltype(Api::submit_cancel)>(dll,"ksa64_viewer_submit_cancel"),
  required<decltype(Api::completed)>(dll,"ksa64_viewer_completed_ksb11"),required<decltype(Api::diagnostic)>(dll,"ksa64_viewer_diagnostic"),optional<decltype(Api::panic_probe)>(dll,"ksa64_viewer_test_panic_probe")};
 assert(a.abi(nullptr)==KSA64_VIEWER_INVALID_ARGUMENT);auto ld=empty_buffer();assert(a.library_diagnostic(&ld)==0&&ld.length>0);assert(a.free_buffer(&ld)==0);
 Ksa64ViewerAbiInfo bad{};bad.abi_version=99;bad.struct_size=sizeof(bad);assert(a.abi(&bad)==KSA64_VIEWER_ABI_MISMATCH);bad.abi_version=1;bad.struct_size=sizeof(bad)-1;assert(a.abi(&bad)==KSA64_VIEWER_STRUCT_SIZE);Ksa64ViewerAbiInfo info{};info.abi_version=1;info.struct_size=sizeof(info);assert(a.abi(&info)==0&&info.snapshot_size==sizeof(Ksa64ViewerSnapshot));
 auto cat=empty_buffer();assert(a.catalog(&cat)==0&&cat.length>100);std::string catalog(reinterpret_cast<char*>(cat.data),static_cast<size_t>(cat.length));assert(catalog.find("ksa64.product-catalog.v1")!=std::string::npos);assert(a.catalog(&cat)==KSA64_VIEWER_INVALID_ARGUMENT);auto duplicate=cat;assert(a.free_buffer(&cat)==0);assert(a.free_buffer(&duplicate)==KSA64_VIEWER_INVALID_ARGUMENT);
 const char role_text[]="guided-operator";auto role=make_span(reinterpret_cast<const uint8_t*>(role_text),sizeof(role_text)-1);Ksa64ViewerHandle*h=nullptr;assert(a.start(&role,&h)==0);auto s=initial_snapshot(a,h);assert(s.role==2&&s.release_epoch==0);auto unchanged=snap_header();unchanged.command_sequence=UINT64_MAX;assert(a.poll(h,&unchanged)==KSA64_VIEWER_UNCHANGED&&unchanged.command_sequence==UINT64_MAX);
 assert(a.resume(h)==1);s=wait_command(a,h,s.command_sequence);assert(s.command_result==KSA64_VIEWER_LIFECYCLE);auto d=empty_buffer();assert(a.diagnostic(h,&d)==0&&d.length>0);assert(a.free_buffer(&d)==0);
 if(a.panic_probe){Ksa64ViewerHandle*ph=nullptr;assert(a.start(&role,&ph)==0);assert(a.panic_probe(ph)==1);auto failed=snap_header();auto end=std::chrono::steady_clock::now()+std::chrono::seconds(5);int32_t r;while((r=a.poll(ph,&failed))!=KSA64_VIEWER_PANIC){assert(r==0||r==KSA64_VIEWER_NO_DATA||r==KSA64_VIEWER_UNCHANGED);assert(std::chrono::steady_clock::now()<end);std::this_thread::yield();}assert(a.resume(ph)==KSA64_VIEWER_PANIC);assert(a.destroy(ph)==0);}
 while(s.lifecycle!=kCompleted){auto load=empty_buffer();int32_t lr=a.recommended(h,&load);assert(lr==0||lr==KSA64_VIEWER_NO_DATA);if(lr==0){assert(load.length==512);auto p=make_span(load.data,load.length);assert(a.submit_stage(h,&p,0)==1);assert(a.free_buffer(&load)==0);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0);auto commit=empty_buffer();assert(a.commit_request(h,&commit)==0&&commit.length==128);p=make_span(commit.data,commit.length);assert(a.submit_commit(h,&p)==1);assert(a.free_buffer(&commit)==0);s=wait_command(a,h,s.command_sequence);assert(s.command_result==0);}auto begin=std::chrono::steady_clock::now();assert(a.advance(h,32)==1);assert(std::chrono::steady_clock::now()-begin<std::chrono::milliseconds(50));s=wait_command(a,h,s.command_sequence);assert(s.command_result==0);}
 assert(s.evidence_identity!=0);auto ksb=empty_buffer();auto end=std::chrono::steady_clock::now()+std::chrono::seconds(5);int32_t kr;while((kr=a.completed(h,&ksb))==KSA64_VIEWER_NO_DATA){assert(std::chrono::steady_clock::now()<end);std::this_thread::yield();}assert(kr==0&&ksb.length==22369&&std::memcmp(ksb.data,"KSB1",4)==0);const std::array<uint8_t,32> expected{0x38,0xa3,0xef,0x2e,0x49,0x7b,0x8e,0x24,0xd1,0xcf,0x53,0xa5,0x6d,0xb8,0x5b,0x3d,0x8b,0xea,0x0b,0xdb,0x27,0x58,0x62,0x15,0xa0,0x2f,0xf7,0x5d,0x0e,0xe3,0x9d,0xc8};assert(sha256(ksb.data,static_cast<size_t>(ksb.length))==expected);assert(a.free_buffer(&ksb)==0);
 drain_events(a,h);assert(a.destroy(h)==0);auto stale=snap_header();assert(a.poll(h,&stale)==KSA64_VIEWER_INVALID_ARGUMENT);cancel_abort(a,role);
 FreeLibrary(dll);std::cout<<"KSA64 viewer ABI harness passed (controls, misuse, event order, frozen KSB11 length and SHA-256 verified)\n";return 0;
}
