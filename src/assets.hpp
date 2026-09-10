// Phase 0 BL2 832/46 asset spikes. Format reference: UEViewer (see THIRD_PARTY.md).
#pragma once
#include <array>
#include <filesystem>

namespace assets {
using Bytes = std::vector<unsigned char>;
void check(bool condition, const char* message) { if (!condition) throw std::runtime_error(message); }
uint16_t u16(Reader& r) { r.require(2); auto v = uint16_t(r.data[r.pos] | (r.data[r.pos+1] << 8)); r.pos += 2; return v; }
float real(Reader& r) { auto f = std::bit_cast<float>(r.u32()); check(std::isfinite(f), "non-finite asset coordinate"); return f; }
size_t count(Reader& r, size_t stride) {
    auto n = r.i32(); check(n >= 0, "negative asset array count");
    check(size_t(n) <= (std::min(r.data.size(), r.limit) - r.pos) / stride, "asset array exceeds payload"); return size_t(n);
}
void bulkArraySkip(Reader& r) { auto stride = r.u32(); check(stride > 0 && stride <= 1024, "invalid bulk array stride"); r.skip(count(r, stride) * stride); }
void write(const std::filesystem::path& path, const Bytes& bytes) {
    std::ofstream out(path, std::ios::binary); check(bool(out), "cannot open asset output");
    out.write(reinterpret_cast<const char*>(bytes.data()), std::streamsize(bytes.size())); check(bool(out), "asset output write failed");
}
void be(Bytes& b, uint32_t v) { for (int i=24;i>=0;i-=8) b.push_back(static_cast<unsigned char>(v>>i)); }
void pngChunk(Bytes& png, const char* type, const Bytes& data) {
    be(png, uint32_t(data.size())); const auto start=png.size();
    png.insert(png.end(),type,type+4); png.insert(png.end(),data.begin(),data.end());
    uint32_t crc=0xffffffff;
    for(size_t i=start;i<png.size();++i) { crc ^= png[i]; for(int j=0;j<8;++j) crc=(crc>>1) ^ ((crc&1)?0xedb88320u:0); }
    be(png,~crc);
}
void png(const std::filesystem::path& path, uint32_t w, uint32_t h, const Bytes& pixels) {
    Bytes rows; rows.reserve(size_t(h)*(size_t(w)*4+1));
    for(size_t y=0;y<h;++y) { rows.push_back(0); rows.insert(rows.end(),pixels.begin()+y*w*4,pixels.begin()+(y+1)*w*4); }
    Bytes z{0x78,0x01};
    for(size_t at=0;at<rows.size();) {
        const auto n=uint16_t(std::min(size_t(65535),rows.size()-at)); z.push_back(at+n==rows.size()?1:0);
        z.push_back(n&255); z.push_back(n>>8); z.push_back((~n)&255); z.push_back(((~n)>>8)&255);
        z.insert(z.end(),rows.begin()+at,rows.begin()+at+n); at+=n;
    }
    uint32_t a=1,b=0; for(auto c:rows) { a=(a+c)%65521; b=(b+a)%65521; } be(z,(b<<16)|a);
    Bytes result{137,80,78,71,13,10,26,10}, header; be(header,w); be(header,h);
    header.insert(header.end(),{8,6,0,0,0}); pngChunk(result,"IHDR",header); pngChunk(result,"IDAT",z); pngChunk(result,"IEND",{}); write(path,result);
}
Bytes dxt(const Bytes& data, uint32_t w, uint32_t h, bool five) {
    const size_t blockBytes=five?16:8;
    check(w && h && w<=16384 && h<=16384 && uint64_t(w)*h<=64u*1024*1024, "invalid texture dimensions or exceeds 256 MiB decoded limit");
    check(data.size()==size_t((w+3)/4)*((h+3)/4)*blockBytes,"DXT mip byte count mismatch");
    Bytes pixels(size_t(w)*h*4); Reader r{data};
    for(uint32_t by=0;by<h;by+=4) for(uint32_t bx=0;bx<w;bx+=4) {
        std::array<unsigned,8> alpha{}; uint64_t alphaBits=0;
        if(five) {
            alpha[0]=r.data[r.pos++]; alpha[1]=r.data[r.pos++];
            for(unsigned i=0;i<6;++i) alphaBits |= uint64_t(r.data[r.pos++])<<(8*i);
            if(alpha[0]>alpha[1]) for(unsigned i=1;i<=6;++i) alpha[i+1]=((7-i)*alpha[0]+i*alpha[1])/7;
            else { for(unsigned i=1;i<=4;++i) alpha[i+1]=((5-i)*alpha[0]+i*alpha[1])/5; alpha[6]=0; alpha[7]=255; }
        }
        auto c0=u16(r),c1=u16(r); std::array<std::array<unsigned,4>,4> colors{};
        for(unsigned i=0;i<2;++i) { auto c=i?c1:c0; colors[i]={unsigned(((c>>11)*255+15)/31),unsigned((((c>>5)&63)*255+31)/63),unsigned(((c&31)*255+15)/31),255}; }
        if(c0>c1 || five) for(unsigned k=0;k<4;++k) { colors[2][k]=(2*colors[0][k]+colors[1][k])/3; colors[3][k]=(colors[0][k]+2*colors[1][k])/3; }
        else { for(unsigned k=0;k<4;++k) colors[2][k]=(colors[0][k]+colors[1][k])/2; colors[3]={0,0,0,0}; }
        auto bits=r.u32();
        for(unsigned i=0;i<16;++i) { auto color=colors[(bits>>(i*2))&3]; if(five) color[3]=alpha[(alphaBits>>(i*3))&7];
            auto x=bx+i%4,y=by+i/4; if(x<w&&y<h) for(unsigned k=0;k<4;++k) pixels[(size_t(y)*w+x)*4+k]=static_cast<unsigned char>(color[k]); }
    }
    return pixels;
}
struct Mip { uint32_t flags, count, stored, offset, w, h; size_t inlineAt; };
std::string texture(const Package& p, Reader& r, int index, size_t offset, const std::filesystem::path& out, const std::filesystem::path& tfc) {
    check(p.object(index).cls && p.object(p.object(index).cls).name=="Texture2D", "export is not Texture2D");
    const auto props=p.properties(r,index,offset); const auto native=r.pos;
    // Read relevant scalar properties again without adding a JSON dependency.
    std::string format, cache; r.pos=size_t(p.object(index).offset)+offset;
    while(true) { auto name=p.name(r); if(name=="None") break; auto type=p.name(r); auto size=r.i32(); r.i32();
        if(type=="StructProperty"||type=="ByteProperty") p.name(r); if(type=="BoolProperty") r.skip(1);
        const auto end=r.pos+size;
        if(name=="Format"&&type=="ByteProperty") format=p.name(r);
        if(name=="TextureFileCacheName"&&type=="NameProperty") cache=p.name(r);
        r.pos=end;
    }
    check(format=="PF_DXT1"||format=="PF_DXT5","texture spike supports PF_DXT1/PF_DXT5");
    r.pos=native; r.skip(16); const auto n=count(r,24); check(n>0&&n<=32,"invalid texture mip count");
    std::vector<Mip> mips;
    for(size_t i=0;i<n;++i) {
        Mip m{}; m.flags=r.u32(); m.count=r.u32(); m.stored=r.u32(); m.offset=r.u32(); m.inlineAt=r.pos;
        if(!(m.flags&1)&&!(m.flags&32)) r.skip(m.stored);
        m.w=r.u32();m.h=r.u32(); mips.push_back(m);
    }
    size_t chosen=0; while(chosen<mips.size() && ((mips[chosen].flags&32)||!mips[chosen].count)) ++chosen;
    check(chosen<mips.size(),"no resident texture mip"); const auto& m=mips[chosen];
    check(!(m.flags&~uint32_t(1|8|16|32)),"unsupported texture bulk codec/flags");
    Bytes payload;
    if(m.flags&1) {
        check(!cache.empty() && cache!="None" && cache.find_first_of("/\\:")==std::string::npos,"invalid/missing TFC cache name");
        std::ifstream file(tfc/(cache+".tfc"),std::ios::binary|std::ios::ate); check(bool(file),"cannot open texture cache");
        auto total=file.tellg(); check(total>=0 && uint64_t(m.offset)+m.stored<=uint64_t(total),"TFC mip exceeds cache");
        check(m.stored<=512u*1024*1024,"TFC mip too large"); payload.resize(m.stored);file.seekg(m.offset);
        file.read(reinterpret_cast<char*>(payload.data()),m.stored);check(bool(file),"TFC read failed");
    } else payload.assign(r.data.begin()+m.inlineAt,r.data.begin()+m.inlineAt+m.stored);
    if(m.flags&16) payload=decode_package(std::move(payload));
    check(payload.size()==m.count,"texture bulk decoded size mismatch");
    png(out,m.w,m.h,dxt(payload,m.w,m.h,format=="PF_DXT5"));
    return "{\"path\":"+quote(p.path(index))+",\"format\":"+quote(format)+",\"width\":"+std::to_string(m.w)+",\"height\":"+std::to_string(m.h)+",\"mip\":"+std::to_string(chosen)+",\"streamed\":"+((m.flags&1)?"true":"false")+"}";
}
float half(uint16_t h) {
    auto exp=(h>>10)&31; auto mant=h&1023;
    check(exp!=31,"non-finite half UV");
    return (h&32768?-1.f:1.f)*(exp?std::ldexp(float(1024+mant),int(exp)-25):std::ldexp(float(mant),-24));
}
std::string mesh(const Package& p, Reader& r, int index, size_t offset, const std::filesystem::path& out) {
    check(p.object(index).cls && p.object(p.object(index).cls).name=="StaticMesh","export is not StaticMesh");
    p.properties(r,index,offset); r.skip(28); auto body=r.i32(); if(body) p.object(body);
    r.skip(24); bulkArraySkip(r); bulkArraySkip(r); r.u32();
    check(r.u32()==0,"extra source LOD unsupported in spike"); r.skip(count(r,7)*7); r.u32();
    const auto lods=count(r,16); check(lods>0,"mesh has no LOD");
    auto flags=r.u32(); r.u32(); auto stored=r.u32(); r.u32(); if(!(flags&1)&&!(flags&32)) r.skip(stored);
    struct Section {int material; uint32_t first,faces,min,max;}; std::vector<Section> sections;
    const auto ns=count(r,41);
    for(size_t i=0;i<ns;++i) { Section s{};s.material=r.i32();if(s.material)p.object(s.material);r.skip(12);s.first=r.u32();s.faces=r.u32();s.min=r.u32();s.max=r.u32();r.u32();r.skip(count(r,8)*8);r.require(1);check(r.data[r.pos++]==0,"PS3 mesh section unsupported");sections.push_back(s); }
    auto stride=r.u32(),nv=r.u32();check(stride==12&&nv>0,"invalid position stream");
    check(r.u32()==12,"invalid position bulk stride");check(count(r,12)==nv,"position count mismatch");
    std::vector<std::array<float,3>> vertices; for(size_t i=0;i<nv;++i) { float x=real(r),y=real(r),z=real(r);vertices.push_back({x,y,z}); }
    auto sets=r.u32(),uvStride=r.u32(),uvCount=r.u32(),full=r.u32();
    check(sets>=1&&sets<=8&&full<=1&&uvCount==nv&&uvStride==8+sets*(full?8:4),"invalid UV stream");
    check(r.u32()==uvStride&&count(r,uvStride)==nv,"UV bulk mismatch");
    std::vector<std::array<float,2>> uvs; std::vector<std::array<float,3>> normals;
    for(size_t i=0;i<nv;++i) {r.skip(4);r.require(4);normals.push_back({float(r.data[r.pos])/127.5f-1,float(r.data[r.pos+1])/127.5f-1,float(r.data[r.pos+2])/127.5f-1});r.skip(4);
        float u=full?real(r):half(u16(r)),v=full?real(r):half(u16(r));uvs.push_back({u,v});r.skip((sets-1)*(full?8:4)); }
    auto colorStride=r.u32(),colors=r.u32();check(colors==0||(colors==nv&&colorStride==4),"invalid color stream");if(colors)bulkArraySkip(r);
    check(r.u32()==nv,"LOD vertex count mismatch");check(r.u32()==2,"spike requires 16-bit mesh indices");auto ni=count(r,2);check(ni%3==0,"non-triangle index count");
    std::vector<uint16_t> indices;for(size_t i=0;i<ni;++i){auto v=u16(r);check(v<nv,"mesh index out of range");indices.push_back(v);}
    std::ostringstream obj;obj<<std::setprecision(9)<<"# OpenWillow local extraction; UE coordinates X,Y,Z; centimeters\n";
    for(auto v:vertices)obj<<"v "<<v[0]<<' '<<v[1]<<' '<<v[2]<<'\n';
    for(auto v:uvs)obj<<"vt "<<v[0]<<' '<<1-v[1]<<'\n';
    for(auto v:normals)obj<<"vn "<<v[0]<<' '<<v[1]<<' '<<v[2]<<'\n';
    std::ostringstream report;report<<"{\"path\":"<<quote(p.path(index))<<",\"vertices\":"<<nv<<",\"triangles\":"<<ni/3<<",\"lods\":"<<lods<<",\"sections\":[";
    for(size_t i=0;i<sections.size();++i){auto s=sections[i];check(s.first<=ni&&s.faces<=(ni-s.first)/3&&s.min<=s.max&&s.max<nv,"invalid mesh section range");
        obj<<"g section_"<<i<<"\n# material "<<(s.material?p.path(s.material):"None")<<'\n';
        if(i)report<<',';report<<"{\"material_index\":"<<s.material<<",\"material\":"<<quote(s.material?p.path(s.material):"None")<<",\"triangles\":"<<s.faces<<'}';
        for(size_t t=s.first;t<s.first+size_t(s.faces)*3;t+=3){obj<<'f';for(size_t k=0;k<3;++k){auto v=indices[t+k]+1;obj<<' '<<v<<'/'<<v<<'/'<<v;}obj<<'\n';}}
    auto str=obj.str();write(out,Bytes(str.begin(),str.end())); report<<"]}";return report.str();
}
} // namespace assets
