#pragma once

#include "CoreMinimal.h"
#include "Widgets/SCompoundWidget.h"

class UKsa64GlobalViewerSubsystem;

class SKsa64GlobalViewerOverlay final : public SCompoundWidget
{
public:
    SLATE_BEGIN_ARGS(SKsa64GlobalViewerOverlay) {}
        SLATE_ARGUMENT(TWeakObjectPtr<UKsa64GlobalViewerSubsystem>, Subsystem)
    SLATE_END_ARGS()

    void Construct(const FArguments& Args);
    virtual bool SupportsKeyboardFocus() const override { return true; }
    virtual FReply OnKeyDown(
        const FGeometry& MyGeometry,
        const FKeyEvent& InKeyEvent) override;

private:
    TSharedRef<SWidget> BuildTopBar();
    TSharedRef<SWidget> BuildEngineeringPanel();
    TSharedRef<SWidget> BuildBottomBar();
    TSharedRef<SWidget> Button(
        TAttribute<FText> Text,
        const FOnClicked& Clicked,
        TAttribute<bool> Enabled = TAttribute<bool>(true)) const;
    FReply OnStart();
    FReply OnLayout();
    FReply OnCamera();
    FReply OnAutomatic();
    FReply OnOperations();
    FReply OnTruth();
    FReply OnPauseResume();
    FReply OnStep();
    EVisibility EngineeringVisibility() const;
    EVisibility BottomVisibility() const;
    FText TruthButtonText() const;
    FText OperationsButtonText() const;
    FText PauseButtonText() const;

    TWeakObjectPtr<UKsa64GlobalViewerSubsystem> Subsystem;
};
