#pragma once

#include "CoreMinimal.h"
#include "Widgets/SCompoundWidget.h"
#include "Widgets/SLeafWidget.h"
#include "Ksa64OperationsTypes.h"

class UKsa64LiveMissionSubsystem;

class SKsa64OperationsPlot final : public SLeafWidget
{
public:
    SLATE_BEGIN_ARGS(SKsa64OperationsPlot) {}
        SLATE_ARGUMENT(TWeakObjectPtr<UKsa64LiveMissionSubsystem>, Subsystem)
    SLATE_END_ARGS()

    void Construct(const FArguments& Args);
    virtual FVector2D ComputeDesiredSize(float LayoutScaleMultiplier) const override;
    virtual int32 OnPaint(
        const FPaintArgs& Args,
        const FGeometry& AllottedGeometry,
        const FSlateRect& MyCullingRect,
        FSlateWindowElementList& OutDrawElements,
        int32 LayerId,
        const FWidgetStyle& InWidgetStyle,
        bool bParentEnabled) const override;

private:
    TWeakObjectPtr<UKsa64LiveMissionSubsystem> Subsystem;
};

class SKsa64OperationsDashboard final : public SCompoundWidget
{
public:
    SLATE_BEGIN_ARGS(SKsa64OperationsDashboard) {}
        SLATE_ARGUMENT(TWeakObjectPtr<UKsa64LiveMissionSubsystem>, Subsystem)
    SLATE_END_ARGS()

    void Construct(const FArguments& Args);
    virtual bool SupportsKeyboardFocus() const override { return true; }
    virtual FReply OnKeyDown(const FGeometry& MyGeometry, const FKeyEvent& InKeyEvent) override;

private:
    TSharedRef<SWidget> BuildHeader();
    TSharedRef<SWidget> BuildTransportControls();
    TSharedRef<SWidget> BuildTrajectoryPanel();
    TSharedRef<SWidget> BuildNavigationPanel();
    TSharedRef<SWidget> BuildTimelinePanel();
    TSharedRef<SWidget> BuildProcedurePanel();
    TSharedRef<SWidget> BuildUplinkPanel();
    TSharedRef<SWidget> BuildDispositionPanel();
    TSharedRef<SWidget> BuildEngineeringPanel();
    TSharedRef<SWidget> BuildAccessibilityControls();
    TSharedRef<SWidget> Panel(
        const FText& Title,
        const TSharedRef<SWidget>& Content,
        const FLinearColor& Accent) const;
    TSharedRef<SWidget> Label(
        TAttribute<FText> Text,
        int32 BaseSize,
        const FLinearColor& Color) const;
    TSharedRef<SWidget> CommandButton(
        TAttribute<FText> Text,
        const FOnClicked& OnClicked,
        TAttribute<bool> Enabled,
        const FLinearColor& Accent) const;

    FReply OnStart();
    FReply OnPauseResume();
    FReply OnStep();
    FReply OnSetPace(EKsa64OperationsPace Pace);
    FReply OnReview();
    FReply OnStage();
    FReply OnCommit();
    FReply OnCancel();
    FReply OnReducedMotion();
    FReply OnHighContrast();
    FReply OnSoundCues();
    FReply OnTextScale();

    FText HeaderMissionText() const;
    FText HeaderStateText() const;
    FText NavigationText() const;
    FText ProcedureText() const;
    FText ProcedureGuardText() const;
    FText UplinkText() const;
    FText TimelineText() const;
    FText DispositionText() const;
    FText EngineeringText() const;
    FText PauseResumeText() const;
    FText AccessibilityText() const;
    bool HasSession() const;
    bool CanStart() const;
    bool CanReviewAction() const;
    bool CanStageAction() const;
    bool CanCommitAction() const;
    bool CanCancelAction() const;
    float TextScale() const;
    FLinearColor PanelBackground() const;
    FLinearColor PrimaryText() const;

    TWeakObjectPtr<UKsa64LiveMissionSubsystem> Subsystem;
};

