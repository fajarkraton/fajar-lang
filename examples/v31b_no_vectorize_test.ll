; ModuleID = 'fj_main'
source_filename = "fj_main"

declare void @fj_rt_println_int(i64) local_unnamed_addr

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define i64 @tight_loop(i64 %0) local_unnamed_addr #0 {
entry:
  %lt11 = icmp sgt i64 %0, 0
  br i1 %lt11, label %while_body.preheader, label %while_after

while_body.preheader:                             ; preds = %entry
  %1 = add nsw i64 %0, -1
  %2 = zext nneg i64 %1 to i65
  %3 = add nsw i64 %0, -2
  %4 = zext i64 %3 to i65
  %5 = mul i65 %2, %4
  %6 = lshr i65 %5, 1
  %7 = trunc i65 %6 to i64
  %8 = add i64 %7, %0
  %9 = mul i64 %8, 37
  %10 = add i64 %9, -37
  br label %while_after

while_after:                                      ; preds = %while_body.preheader, %entry
  %sum.0.lcssa = phi i64 [ 0, %entry ], [ %10, %while_body.preheader ]
  ret i64 %sum.0.lcssa
}

define void @main() local_unnamed_addr {
entry:
  %tight_loop_result = tail call i64 @tight_loop(i64 1152)
  tail call void @fj_rt_println_int(i64 %tight_loop_result)
  ret void
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "no-implicit-float"="true" "target-features"="-avx,-avx2,-avx512f,-sse3,-ssse3,-sse4.1,-sse4.2,+popcnt" }
