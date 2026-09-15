"""Synthetic BSP fixtures only; no installed-package bytes."""
import struct
import sys
import unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from bsp_decode import decode_model,decode_component,validate_coverage,polygon_triangles
from prepare_bsp import section_obj,probe_candidates

def pack(fmt,*values): return struct.pack('<'+fmt,*values)
def bulk(stride,rows): return pack('II',stride,len(rows))+b''.join(rows)

def fixture():
    points=[(0.,0.,0.),(400.,0.,0.),(400.,400.,0.),(0.,400.,0.)]
    plane=(0.,0.,1.,0.)
    tail=bytes(28)+bulk(12,[pack('3f',0,0,1)])+bulk(12,[pack('3f',*p) for p in points])
    tail+=bulk(64,[pack('4f12i',*plane,0,0,0,0,0,-1,-1,-1,-1,4<<16,-1,-1)])
    tail+=pack('II',2,1)+pack('8i4f3i',7,0,0,0,0,0,0,0,*plane,0,0,0)
    tail+=bulk(24,[pack('2i4f',i,-1,0,0,0,0) for i in range(4)])
    model=bytes(12)+tail
    ct=pack('III',2,1,1)+pack('IiiiHII',0,3,7,1,0,0,0)+pack('HIH',0,1,0)
    component=bytes(16)+ct
    def record(i,kind,prefix,payload):
        return {'index':i,'path':'TheWorld.PersistentLevel.'+kind+'_0','class':'Engine.'+kind,'outer':1,
                'data':{'property_offset':prefix,'consumed_bytes':8,'trailing_bytes':len(payload)-prefix-8}}
    records={1:{'index':1,'class':'Engine.Level','path':'TheWorld.PersistentLevel'},
             2:record(2,'Model',4,model),3:record(3,'ModelComponent',8,component)}
    return model,component,records

class BspTests(unittest.TestCase):
    def test_full_cross_checks_and_obj(self):
        b,c,r=fixture();m=decode_model(b,r[2],r);component=decode_component(c,r[3],r,m)
        validate_coverage(m,[component])
        self.assertEqual(m['polygons'][0]['triangles'],[(0,1,2),(0,2,3)])
        obj,n=section_obj(m['polygons']);self.assertEqual(n,2)
        self.assertIn('f 1/1/1 3/3/3 2/2/2',obj)
        self.assertEqual(len(probe_candidates(m,[component],'Synthetic_P')),1)

    def test_every_truncation(self):
        b,c,r=fixture();m=decode_model(b,r[2],r)
        for end in range(12,len(b)):
            r[2]['data']['trailing_bytes']=end-12
            with self.assertRaises(ValueError):decode_model(b[:end],r[2],r)
        for end in range(16,len(c)):
            r[3]['data']['trailing_bytes']=end-16
            with self.assertRaises(ValueError):decode_component(c[:end],r[3],r,m)

    def test_root_ownership(self):
        b,c,r=fixture();r[1]['class']='Engine.BlockingVolume'
        with self.assertRaises(ValueError):decode_model(b,r[2],r)

    def test_bad_stride_count_and_reference(self):
        b,c,r=fixture()
        for offset,value in [(40,16),(44,0xffffffff),(len(b)-96,99)]:
            changed=bytearray(b);struct.pack_into('<I',changed,offset,value)
            with self.assertRaises(ValueError):decode_model(changed,r[2],r)

    def test_component_rejection_and_coverage(self):
        b,c,r=fixture();m=decode_model(b,r[2],r)
        for offset,value in [(16,9),(28,1),(32,8),(36,9),(40,100)]:
            changed=bytearray(c);struct.pack_into('<I',changed,offset,value)
            with self.assertRaises(ValueError):decode_component(changed,r[3],r,m)
        component=decode_component(c,r[3],r,m)
        with self.assertRaises(ValueError):validate_coverage(m,[])
        with self.assertRaises(ValueError):validate_coverage(m,[component,component])
        r[3]['data']['trailing_bytes']+=1
        with self.assertRaises(ValueError):decode_component(c+b'X',r[3],r,m)

    def test_polygon_rejections_and_collinearity(self):
        plane=(0,0,1,0)
        valid=[(0,0,0),(1,0,0),(2,0,0),(2,2,0),(0,2,0)]
        self.assertEqual(len(polygon_triangles(valid,plane)),2)
        for points in [valid[::-1],[(0,0,0),(2,0,0),(1,1,0),(2,2,0),(0,2,0)],
                       [(0,0,0),(2,0,1),(2,2,0)],[(0,0,0)]*3,
                       [(0,0,0),(float('nan'),0,0),(0,1,0)]]:
            with self.assertRaises(ValueError):polygon_triangles(points,plane)

if __name__=='__main__': unittest.main()
