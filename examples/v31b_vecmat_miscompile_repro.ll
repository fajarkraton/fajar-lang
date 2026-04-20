; ModuleID = 'fj_main'
source_filename = "fj_main"

@__fj_str_1 = constant [51 x i8] c"=== V31.B.P0.1 vecmat LLVM O2 miscompile repro ===\00"
@__fj_str_2 = constant [5 x i8] c"m = \00"
@__fj_str_3 = constant [5 x i8] c"n = \00"
@__fj_str_4 = constant [13 x i8] c"sum_total = \00"

declare void @fj_rt_print_str(ptr, i64) local_unnamed_addr

declare void @fj_rt_println_int(i64) local_unnamed_addr

declare void @fj_rt_println_str(ptr, i64) local_unnamed_addr

define void @main() local_unnamed_addr {
entry:
  br label %while_body

while_body:                                       ; preds = %entry, %while_after7
  %j.065 = phi i64 [ 0, %entry ], [ %add47, %while_after7 ]
  %sum_total.064 = phi i64 [ 0, %entry ], [ %add45, %while_after7 ]
  br label %while_body6

while_after:                                      ; preds = %while_after7
  tail call void @fj_rt_println_str(ptr nonnull @__fj_str_1, i64 50)
  tail call void @fj_rt_print_str(ptr nonnull @__fj_str_2, i64 4)
  tail call void @fj_rt_println_int(i64 1152)
  tail call void @fj_rt_print_str(ptr nonnull @__fj_str_3, i64 4)
  tail call void @fj_rt_println_int(i64 6912)
  tail call void @fj_rt_print_str(ptr nonnull @__fj_str_4, i64 12)
  tail call void @fj_rt_println_int(i64 %add45)
  ret void

while_body6:                                      ; preds = %while_body, %while_body6
  %storemerge63 = phi i64 [ 0, %while_body ], [ %add42, %while_body6 ]
  %add406162 = phi i64 [ 0, %while_body ], [ %add40, %while_body6 ]
  %mul13 = mul nuw nsw i64 %storemerge63, 2816
  %add = add nuw nsw i64 %mul13, %j.065
  %mul16 = mul nuw nsw i64 %storemerge63, 5
  %add19 = add nuw nsw i64 %mul16, %j.065
  %and = and i64 %add19, 15
  %shr = lshr i64 %add, 7
  %and23 = and i64 %shr, 31
  %mul24 = mul nuw nsw i64 %and23, 100
  %add25 = add nuw nsw i64 %mul24, 500
  %sub = add nuw nsw i64 %and, 4294967289
  %0 = mul nuw nsw i64 %storemerge63, 100
  %mul33 = add nuw i64 %0, 4294909696
  %mul29 = mul i64 %sub, %mul33
  %mul37 = mul i64 %mul29, %add25
  %div39.lhs.trunc = trunc i64 %mul37 to i32
  %div3966 = sdiv i32 %div39.lhs.trunc, 1000000
  %div39.sext = sext i32 %div3966 to i64
  %add40 = add i64 %add406162, %div39.sext
  %add42 = add nuw nsw i64 %storemerge63, 1
  %lt10 = icmp ult i64 %storemerge63, 1151
  br i1 %lt10, label %while_body6, label %while_after7

while_after7:                                     ; preds = %while_body6
  %add45 = add i64 %add40, %sum_total.064
  %add47 = add nuw nsw i64 %j.065, 1
  %lt = icmp ult i64 %j.065, 6911
  br i1 %lt, label %while_body, label %while_after
}
