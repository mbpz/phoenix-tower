import copy
import json
from pathlib import Path
import unittest
from l3_section_registration import validate_scope

ROOT = Path(__file__).resolve().parents[2]

class RegistrationScopeTests(unittest.TestCase):
    def setUp(self):
        self.scope = json.loads((ROOT/'docs/refactoring/yellow-crane-l3-section-registration.json').read_text())
        self.plan = json.loads((ROOT/'docs/refactoring/yellow-crane-plan-traces.json').read_text())

    def test_source_axis_order_and_diagnostic_are_distinct(self):
        result = validate_scope(self.scope, self.plan)
        self.assertEqual(result, [15,11,7,3,-3,-7,-11,-15])
        self.assertEqual(self.scope['diagnostic_cut']['x_m'],3)

    def test_rejects_registration_promotion_or_invented_path(self):
        for key,value in [('registration_verified',True),('resolved_path_xy_m',[[-2,15],[-2,-15]])]:
            data=copy.deepcopy(self.scope)
            data['source_section'][key]=value
            with self.assertRaises(ValueError): validate_scope(data,self.plan)
        for key in ['export_to_game','transfer_to_exterior']:
            data=copy.deepcopy(self.scope); data[key]=True
            with self.assertRaises(ValueError): validate_scope(data,self.plan)

    def test_rejects_source_axis_reversal_and_wrong_probe_side(self):
        data=copy.deepcopy(self.scope)
        data['source_section']['horizontal_axes_left_to_right'].reverse()
        with self.assertRaises(ValueError): validate_scope(data,self.plan)
        data=copy.deepcopy(self.scope); data['sensitivity_probes'][0]['y_interval_m']=[8,15]
        with self.assertRaises(ValueError): validate_scope(data,self.plan)

    def test_rejects_missing_uncertainty_and_nonfinite_plane(self):
        for value in [True,None,0]:
            data=copy.deepcopy(self.scope); data['sensitivity_probes'][0]['registered_to_source']=value
            with self.assertRaises(ValueError): validate_scope(data,self.plan)
        for value in [float('nan'),float('inf'),3.]:
            data=copy.deepcopy(self.scope); data['sensitivity_probes'][0]['x_m']=value
            with self.assertRaises(ValueError): validate_scope(data,self.plan)

    def test_rejects_promoted_kind_and_reserved_control_id(self):
        data=copy.deepcopy(self.scope)
        data['diagnostic_cut']['kind']='registered_source_section'
        with self.assertRaises(ValueError): validate_scope(data,self.plan)
        data=copy.deepcopy(self.scope)
        data['sensitivity_probes'][0]['id']='study_01_control'
        with self.assertRaises(ValueError): validate_scope(data,self.plan)
