#include "OpenWillowCollision.h"
#include "Engine/StaticMesh.h"
#include "PhysicsEngine/BodySetup.h"

TArray<FOpenWillowHull> UOpenWillowCollision::GetHulls(UStaticMesh* Mesh)
{
    TArray<FOpenWillowHull> Result;
    if (Mesh && Mesh->GetBodySetup())
        for (const auto& Elem : Mesh->GetBodySetup()->AggGeom.ConvexElems)
        {
            FOpenWillowHull Hull;
            Hull.Vertices = Elem.VertexData;
            Result.Add(MoveTemp(Hull));
        }
    return Result;
}

bool UOpenWillowCollision::SetHulls(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls)
{
    if (!Mesh || Hulls.Num() > 4096) return false;
    for (const auto& Hull : Hulls)
    {
        if (Hull.Vertices.Num() < 4 || Hull.Vertices.Num() > 4096) return false;
        for (const FVector& Vertex : Hull.Vertices)
            if (Vertex.ContainsNaN()) return false;
    }
    Mesh->CreateBodySetup();
    UBodySetup* Body = Mesh->GetBodySetup();
    Body->Modify();
    Body->RemoveSimpleCollision();
    Body->CollisionTraceFlag = CTF_UseSimpleAsComplex;
    for (const auto& Hull : Hulls)
    {
        FKConvexElem Elem;
        Elem.VertexData = Hull.Vertices;
        Elem.UpdateElemBox();
        Body->AggGeom.ConvexElems.Add(MoveTemp(Elem));
    }
    Body->InvalidatePhysicsData();
    Body->CreatePhysicsMeshes();
    for (const auto& Elem : Body->AggGeom.ConvexElems)
        if (!Elem.GetChaosConvexMesh()) return false;
    Mesh->MarkPackageDirty();
    return true;
}
