import hashlib, os, sys, unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from patch_game import patch, OFFSET

class GamePatchTests(unittest.TestCase):
    def test_reject_unsupported(self):
        with self.assertRaises(ValueError): patch(b'not the supported SWF')
    @unittest.skipUnless(os.environ.get('DREAM_ORIGINAL_SWF'),'Owner fixture unavailable')
    def test_owner_fixture(self):
        original=Path(os.environ['DREAM_ORIGINAL_SWF']).read_bytes();result=patch(original)
        self.assertEqual(len(original),len(result))
        self.assertEqual([i for i,(a,b) in enumerate(zip(original,result)) if a!=b],[OFFSET+6])
        self.assertEqual(hashlib.sha256(result).hexdigest(),'b790348ffa130600ebe52588aab1762e75a14450041b66fa1b46720b9b37bd81')
        with self.assertRaises(ValueError): patch(original[:-1])
        with self.assertRaises(ValueError): patch(result)

if __name__=='__main__': unittest.main()
