import os,subprocess,tempfile,unittest
from pathlib import Path
HELPER=Path(__file__).resolve().parents[1]/'port/dreammachine/gpu-governor.sh'
class GPUGovernorTests(unittest.TestCase):
    def run_shell(self, body, setting='1', available='simple_ondemand performance\n'):
        with tempfile.TemporaryDirectory(dir=os.environ.get('TMPDIR')) as t:
            p=Path(t);(p/'governor').write_text('simple_ondemand\n');(p/'available_governors').write_text(available)
            env=dict(os.environ,DREAM_GPU_SYSFS=t,DREAM_GPU_PERFORMANCE=setting)
            r=subprocess.run(['bash','-c',f'source "$1"; {body}','test',str(HELPER)],env=env,text=True,capture_output=True)
            return r,(p/'governor').read_text().strip()
    def test_enable_then_exit_restores(self):
        r,gov=self.run_shell('trap dream_gpu_restore EXIT; dream_gpu_enable; [[ $(<"$DREAM_GPU_SYSFS/governor") == performance ]]')
        self.assertEqual(r.returncode,0,r.stderr);self.assertEqual(gov,'simple_ondemand');self.assertIn('restored=simple_ondemand',r.stdout)
    def test_disable_preserves_original(self):
        r,gov=self.run_shell('dream_gpu_enable; dream_gpu_restore','0');self.assertEqual(r.returncode,0);self.assertEqual(gov,'simple_ondemand');self.assertNotIn('governor=performance',r.stdout)
    def test_unavailable_governor_is_noop(self):
        r,gov=self.run_shell('dream_gpu_enable; dream_gpu_restore',available='simple_ondemand\n');self.assertEqual(r.returncode,0);self.assertEqual(gov,'simple_ondemand')
    def test_external_mutation_is_preserved(self):
        r,gov=self.run_shell('dream_gpu_enable; printf "userspace\\n" > "$DREAM_GPU_SYSFS/governor"; dream_gpu_restore');self.assertEqual(r.returncode,0);self.assertEqual(gov,'userspace');self.assertIn('skipped',r.stdout)
if __name__=='__main__':unittest.main()
